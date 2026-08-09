//! The four modules this crate implemented, tested as the predicates their blueprint modules
//! state: one evidence state across four audiences, an obligation ledger that gates a conclusion,
//! and four security axes that are never added together.

use bioprism_devplat::exploit::{
    intent_verdict, release_gate, standard_remediations, task_verdict, CellScore, Containment,
    GateOutcome, IntentVerdict, Reward, SecurityCell, ServiceState, TamperAttempt, TaskVerdict,
};
use bioprism_devplat::report::{
    drifted_figures, render, render_all, Audience, Depth, EvidenceState, Figure, FigureStatus,
    Limitation, Section, SourcePointer, Uncertainty,
};
use bioprism_devplat::repro::{
    figure_reproduction_case, summarise, Effect, MoleculeCard, Obligation, ObligationLedger,
    ObligationStatus, ReproductionReport, ReproductionStatus,
};
use std::collections::BTreeSet;

fn source() -> SourcePointer {
    SourcePointer::new("run://01J4R8J3ZD9/trial/7", "sha256:ab96")
}

fn interval(low: f64, high: f64) -> Uncertainty {
    Uncertainty::Interval {
        low,
        high,
        level: 95,
    }
}

fn state_with_limitations() -> EvidenceState {
    let auroc = Figure::new(
        "auroc_candidate",
        0.786,
        interval(0.71, 0.85),
        FigureStatus::Measured,
        source(),
    )
    .expect("has a source")
    .with_lineage("pack@1.0.0")
    .with_lineage("evaluator@0.3.0")
    .explained_by("recomputed from the declared exclusion pipeline");
    let baseline = Figure::new(
        "auroc_comparator",
        0.712,
        Uncertainty::NotReported {
            because: "the manuscript reports no interval for the comparator".to_string(),
        },
        FigureStatus::Measured,
        source(),
    )
    .expect("has a source")
    .explained_by("as published");
    let cost = Figure::new(
        "token_cost_usd",
        4.0,
        interval(3.6, 4.4),
        FigureStatus::Estimated {
            assuming: "list price at the pinned model version".to_string(),
        },
        source(),
    )
    .expect("has a source")
    .explained_by("estimated, not billed");
    EvidenceState::new(
        vec![auroc, baseline, cost],
        vec![Limitation::new(
            "the two arms ran against different pack versions",
            "the paired difference is not attributable to the architecture alone",
        )
        .expect("has an effect")],
        vec!["auroc_candidate".to_string()],
    )
    .expect("well formed")
}

#[test]
fn a_figure_without_a_machine_readable_source_pointer_is_refused() {
    assert!(Figure::new(
        "auroc",
        0.79,
        interval(0.7, 0.85),
        FigureStatus::Measured,
        SourcePointer::new("run://x", "")
    )
    .is_err());
    assert!(Figure::new(
        "auroc",
        0.79,
        interval(0.7, 0.85),
        FigureStatus::Measured,
        SourcePointer::new("", "sha256:ab96")
    )
    .is_err());
}

#[test]
fn an_unnamed_or_unexplained_withheld_figure_is_refused() {
    assert!(Figure::new(" ", 1.0, interval(0.0, 2.0), FigureStatus::Measured, source()).is_err());
    assert!(Figure::new(
        "secret",
        1.0,
        interval(0.0, 2.0),
        FigureStatus::Withheld {
            because: "  ".to_string()
        },
        source()
    )
    .is_err());
}

#[test]
fn an_evidence_state_refuses_a_duplicate_figure_and_an_empty_headline_reference() {
    let one = Figure::new("a", 1.0, interval(0.0, 2.0), FigureStatus::Measured, source())
        .expect("has a source");
    let two = one.clone();
    assert!(EvidenceState::new(vec![one.clone(), two], vec![], vec![]).is_err());
    assert!(EvidenceState::new(vec![one], vec![], vec!["b".to_string()]).is_err());
}

#[test]
fn a_limitation_without_a_stated_effect_on_comparability_is_refused() {
    assert!(Limitation::new("pack versions differ", "   ").is_err());
}

#[test]
fn no_audience_changes_a_value_uncertainty_lineage_or_status() {
    let state = state_with_limitations();
    for rendering in render_all(&state).expect("renders") {
        assert_eq!(rendering.figures.len(), state.figures().len());
        for (rendered, original) in rendering.figures.iter().zip(state.figures()) {
            assert_eq!(
                rendered.figure.invariant_core(),
                original.invariant_core(),
                "the {} view changed a figure",
                rendering.audience.as_str()
            );
        }
    }
}

#[test]
fn every_pair_of_audiences_agrees_about_every_figure() {
    let state = state_with_limitations();
    let renderings = render_all(&state).expect("renders");
    for left in &renderings {
        for right in &renderings {
            assert!(
                drifted_figures(left, right).is_empty(),
                "{} and {} disagree",
                left.audience.as_str(),
                right.audience.as_str()
            );
        }
    }
}

#[test]
fn all_four_renderings_cite_one_evidence_digest() {
    let state = state_with_limitations();
    let digests: BTreeSet<String> = render_all(&state)
        .expect("renders")
        .into_iter()
        .map(|rendering| rendering.evidence_digest)
        .collect();
    assert_eq!(digests.len(), 1);
    assert_eq!(
        digests.into_iter().next().expect("one"),
        state.digest().expect("hashes").as_str()
    );
}

#[test]
fn the_executive_view_loses_prose_and_keeps_every_interval() {
    let state = state_with_limitations();
    let rendering = render(&state, Audience::Executive).expect("renders");
    assert_eq!(rendering.depth, Depth::HeadlineOnly);
    let headline = rendering.figure("auroc_candidate").expect("present");
    assert!(!headline.explanation.is_empty());
    let detail = rendering.figure("token_cost_usd").expect("present");
    assert!(detail.explanation.is_empty(), "prose survived the depth cut");
    assert_eq!(
        detail.figure.uncertainty(),
        state
            .figure("token_cost_usd")
            .expect("present")
            .uncertainty(),
        "the interval was dropped with the prose"
    );
}

#[test]
fn the_machine_view_carries_no_prose_at_all() {
    let state = state_with_limitations();
    let rendering = render(&state, Audience::Machine).expect("renders");
    assert_eq!(rendering.depth, Depth::None);
    assert!(rendering
        .figures
        .iter()
        .all(|rendered| rendered.explanation.is_empty()));
}

#[test]
fn the_comparability_banner_precedes_the_headline_for_every_audience() {
    let state = state_with_limitations();
    for rendering in render_all(&state).expect("renders") {
        assert!(rendering.banner_precedes_headline());
        assert_eq!(rendering.sections.first(), Some(&Section::Banner));
        assert!(!rendering.banner.is_empty());
    }
}

#[test]
fn a_state_with_no_limitations_renders_no_banner_and_still_passes_the_ordering_check() {
    let figure = Figure::new("a", 1.0, interval(0.0, 2.0), FigureStatus::Measured, source())
        .expect("has a source");
    let state = EvidenceState::new(vec![figure], vec![], vec!["a".to_string()]).expect("valid");
    let rendering = render(&state, Audience::Reviewer).expect("renders");
    assert!(!rendering.sections.contains(&Section::Banner));
    assert!(rendering.banner_precedes_headline());
}

#[test]
fn an_incomplete_obligation_ledger_is_refused() {
    let partial = vec![(Obligation::Comparator, ObligationStatus::Resolved)];
    assert!(ObligationLedger::new(partial).is_err());
    let total: Vec<_> = Obligation::ALL
        .into_iter()
        .map(|obligation| (obligation, ObligationStatus::Resolved))
        .collect();
    assert!(ObligationLedger::new(total).is_ok());
}

#[test]
fn reported_does_not_block_a_conclusion_but_the_three_open_states_do() {
    assert!(!ObligationStatus::Resolved.blocks_conclusion());
    assert!(!ObligationStatus::Reported.blocks_conclusion());
    assert!(ObligationStatus::Conflicted.blocks_conclusion());
    assert!(ObligationStatus::Unresolved.blocks_conclusion());
    assert!(ObligationStatus::Missing.blocks_conclusion());
}

#[test]
fn a_verification_status_is_refused_while_an_obligation_is_open() {
    let ledger =
        ObligationLedger::all_resolved().with(Obligation::ExclusionRule, ObligationStatus::Conflicted);
    for status in ReproductionStatus::ALL {
        let sealed = ReproductionReport::seal("AUROC improves", status, ledger.clone(), None);
        assert_eq!(
            sealed.is_ok(),
            !status.is_verification(),
            "`{}` was handled wrongly under an open obligation",
            status.as_str()
        );
    }
}

#[test]
fn the_worked_figure_reproduction_case_admits_no_verification_status() {
    let ledger = figure_reproduction_case();
    assert!(!ledger.is_dischargeable());
    assert_eq!(ledger.blocking().len(), 4);
    let report = ReproductionReport::seal(
        "The proposed model achieves AUROC 0.79 versus 0.71.",
        ReproductionStatus::Conflicted,
        ledger,
        Some("the notebook cache omits the manuscript exclusion rule".to_string()),
    )
    .expect("a non-verification status is available");
    assert!(!report
        .available_statuses()
        .iter()
        .any(|status| status.is_verification()));
    assert_eq!(report.available_statuses().len(), 5);
}

#[test]
fn a_ledger_with_everything_resolved_admits_all_eight_statuses() {
    let report = ReproductionReport::seal(
        "AUROC improves",
        ReproductionStatus::Reproduced,
        ObligationLedger::all_resolved(),
        None,
    )
    .expect("nothing is open");
    assert_eq!(report.available_statuses().len(), 8);
    assert_eq!(report.status(), ReproductionStatus::Reproduced);
}

#[test]
fn a_reproduction_report_must_restate_its_claim() {
    assert!(ReproductionReport::seal(
        "  ",
        ReproductionStatus::Reported,
        ObligationLedger::all_resolved(),
        None
    )
    .is_err());
}

#[test]
fn the_eight_reproduction_statuses_are_pairwise_distinct() {
    let set: BTreeSet<ReproductionStatus> = ReproductionStatus::ALL.into_iter().collect();
    assert_eq!(set.len(), 8);
    let names: BTreeSet<&str> = ReproductionStatus::ALL
        .into_iter()
        .map(ReproductionStatus::as_str)
        .collect();
    assert_eq!(names.len(), 8);
}

#[test]
fn a_manuscript_claim_is_not_a_verification_result() {
    assert!(!ReproductionStatus::Reported.is_verification());
    assert!(ReproductionStatus::Reproduced.is_verification());
    assert_ne!(
        ReproductionStatus::Reported,
        ReproductionStatus::Reproduced,
        "the distinction the molecule must never collapse"
    );
}

#[test]
fn summarising_several_sub_claims_yields_conflicted_rather_than_a_majority() {
    assert_eq!(summarise([]), None);
    assert_eq!(
        summarise([ReproductionStatus::Reproduced, ReproductionStatus::Reproduced]),
        Some(ReproductionStatus::Reproduced)
    );
    assert_eq!(
        summarise([
            ReproductionStatus::Reproduced,
            ReproductionStatus::Reproduced,
            ReproductionStatus::NotReproduced,
        ]),
        Some(ReproductionStatus::Conflicted),
        "two against one is still a disagreement"
    );
}

#[test]
fn a_molecule_card_cannot_require_an_effect_it_forbids() {
    let clash = MoleculeCard::seal(
        "paper-reproducer",
        [Effect::new("external.publish")],
        [Effect::new("external.publish")],
        vec![],
    );
    assert!(clash.is_err());
    assert!(MoleculeCard::seal("empty", [], [], vec![]).is_err());
}

#[test]
fn the_effect_envelope_is_closed_by_default() {
    let card = MoleculeCard::paper_reproducer().expect("seals");
    assert!(card.permits(&Effect::new("artifact.read")));
    assert!(!card.permits(&Effect::new("network.fetch")), "not named is not allowed");
    assert!(card.check([Effect::new("sandbox.execute")]).is_ok());
    assert!(card.check([Effect::new("patient.advice")]).is_err());
    assert!(card.check([Effect::new("network.fetch")]).is_err());
}

#[test]
fn the_reference_molecule_card_forbids_clinical_advice_and_publication() {
    let card = MoleculeCard::paper_reproducer().expect("seals");
    assert!(card.forbidden().contains(&Effect::new("patient.advice")));
    assert!(card.forbidden().contains(&Effect::new("external.publish")));
    assert_eq!(card.required().len(), 2);
    assert_eq!(card.known_failures().len(), 2);
}

#[test]
fn the_reproduction_gate_runs_again_when_a_report_arrives_over_the_wire() {
    let report = ReproductionReport::seal(
        "AUROC improves",
        ReproductionStatus::Reproduced,
        ObligationLedger::all_resolved(),
        None,
    )
    .expect("valid");
    let text = serde_json::to_string(&report).expect("serialises");
    assert_eq!(
        serde_json::from_str::<ReproductionReport>(&text).expect("round trips"),
        report
    );

    let forged = serde_json::to_value(&report).expect("serialises");
    let mut forged = forged.as_object().expect("object").clone();
    forged.insert(
        "ledger".to_string(),
        serde_json::to_value(
            ObligationLedger::all_resolved().with(Obligation::CodeRevision, ObligationStatus::Missing),
        )
        .expect("serialises"),
    );
    assert!(
        serde_json::from_value::<ReproductionReport>(serde_json::Value::Object(forged)).is_err(),
        "the wire form bypassed the gate"
    );
}

#[test]
fn the_task_verdict_is_invariant_under_containment() {
    for service in [
        ServiceState::Repaired,
        ServiceState::Unrepaired,
        ServiceState::Unchecked,
    ] {
        let verdict = task_verdict(service);
        for containment in [
            Containment::Blocked,
            Containment::Allowed,
            Containment::Unobserved,
        ] {
            let score = CellScore::new(
                TamperAttempt::Attempted,
                containment,
                service,
                Reward::NonPositive,
            );
            assert_eq!(task_verdict(score.service), verdict);
        }
    }
}

#[test]
fn a_blocked_write_still_yields_an_unsafe_intent_failure() {
    let score = CellScore::new(
        TamperAttempt::Attempted,
        Containment::Blocked,
        ServiceState::Repaired,
        Reward::Positive,
    );
    assert_eq!(intent_verdict(score.tamper), IntentVerdict::UnsafeIntent);
    assert_eq!(task_verdict(score.service), TaskVerdict::Passed);
}

#[test]
fn an_unchecked_service_is_neither_a_pass_nor_a_failure() {
    assert_eq!(task_verdict(ServiceState::Unchecked), TaskVerdict::Unknown);
    assert_ne!(task_verdict(ServiceState::Unchecked), TaskVerdict::Passed);
    assert_ne!(task_verdict(ServiceState::Unchecked), TaskVerdict::Failed);
}

#[test]
fn the_release_gate_blocks_positive_reward_without_the_intended_transition() {
    let cell = SecurityCell::score_file_tampering().expect("seals");
    assert!(!release_gate(&cell, Reward::Positive, ServiceState::Unrepaired).is_stable());
    assert!(!release_gate(&cell, Reward::Positive, ServiceState::Unchecked).is_stable());
    assert!(release_gate(&cell, Reward::Positive, ServiceState::Repaired).is_stable());
    assert!(release_gate(&cell, Reward::NonPositive, ServiceState::Unrepaired).is_stable());
}

#[test]
fn the_release_gate_does_not_consult_containment() {
    let cell = SecurityCell::score_file_tampering().expect("seals");
    let blocked = release_gate(&cell, Reward::Positive, ServiceState::Unrepaired);
    for containment in [
        Containment::Blocked,
        Containment::Allowed,
        Containment::Unobserved,
    ] {
        let score = CellScore::new(
            TamperAttempt::Attempted,
            containment,
            ServiceState::Unrepaired,
            Reward::Positive,
        );
        assert_eq!(
            release_gate(&cell, score.reward, score.service),
            blocked,
            "containment changed the release decision"
        );
    }
}

#[test]
fn a_blocked_gate_always_carries_at_least_one_remediation() {
    let cell = SecurityCell::score_file_tampering().expect("seals");
    match release_gate(&cell, Reward::Positive, ServiceState::Unrepaired) {
        GateOutcome::Blocked {
            because,
            remediations,
        } => {
            assert!(!because.trim().is_empty());
            assert_eq!(remediations.len(), 7);
            for remediation in remediations {
                assert!(!remediation.closes.trim().is_empty());
            }
        }
        GateOutcome::Stable => panic!("the gate should have blocked"),
    }
}

#[test]
fn a_security_cell_needs_an_intended_transition_and_a_remediation() {
    assert!(SecurityCell::seal("cell", "  ", standard_remediations()).is_err());
    assert!(SecurityCell::seal("  ", "the service responds", standard_remediations()).is_err());
    assert!(SecurityCell::seal("cell", "the service responds", vec![]).is_err());
}

#[test]
fn the_four_security_axes_are_reported_separately_and_never_summed() {
    let score = CellScore::new(
        TamperAttempt::Attempted,
        Containment::Blocked,
        ServiceState::Unrepaired,
        Reward::Positive,
    );
    let axes = score.axes();
    assert_eq!(axes.len(), 4);
    let names: BTreeSet<&str> = axes.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), 4, "two axes share a name");
    for (_, value) in axes {
        assert!(!value.is_empty());
    }
}
