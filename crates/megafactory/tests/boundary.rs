//! Invariants of the BioDecision compiler and boundary detection, blueprint 35.07.

use bioprism_megafactory::{
    compile_cell_spans, Boundary, BoundaryAgreement, BoundaryError, BoundaryKind, CaptureSession,
    CompressionReport, Field, ReferenceBoundaries, Replayable, Span, SpanKind,
};
use bioprism_scale::audit::{Auditor, ReleaseAudit};

fn session(seqs: &[u64]) -> CaptureSession {
    let mut session = CaptureSession::new("run");
    for seq in seqs {
        session
            .append(Span::new(*seq, SpanKind::Execution).with("cell", Field::recorded("step")))
            .expect("appended in order");
    }
    session
}

fn reference(boundaries: Vec<Boundary>) -> ReferenceBoundaries {
    ReferenceBoundaries::new("reviewer-k", boundaries).expect("an attributed reference")
}

#[test]
fn the_seven_boundary_kinds_are_the_modules_seven_required_components() {
    assert_eq!(BoundaryKind::ALL.len(), 7);
    let names: Vec<&str> = BoundaryKind::ALL.iter().map(|kind| kind.as_str()).collect();
    for expected in [
        "goal_or_obligation_change",
        "evidence_acquisition",
        "analysis_selection",
        "claim_creation_or_revision",
        "resource_expenditure",
        "handoff_or_escalation",
        "irreversible_or_high_regret_action",
    ] {
        assert!(names.contains(&expected), "{expected} is missing");
    }
}

#[test]
fn only_the_irreversible_kind_is_high_regret() {
    let high_regret: Vec<BoundaryKind> = BoundaryKind::ALL
        .into_iter()
        .filter(|kind| kind.is_high_regret())
        .collect();
    assert_eq!(
        high_regret,
        vec![BoundaryKind::IrreversibleOrHighRegretAction]
    );
}

#[test]
fn an_unattributed_reference_is_refused() {
    assert_eq!(
        ReferenceBoundaries::new("  ", Vec::new()),
        Err(BoundaryError::UnattributedReference)
    );
}

#[test]
fn a_repeated_reference_boundary_is_refused() {
    let error = ReferenceBoundaries::new(
        "reviewer-k",
        vec![
            Boundary::new(3, BoundaryKind::EvidenceAcquisition),
            Boundary::new(3, BoundaryKind::AnalysisSelection),
        ],
    )
    .expect_err("two boundaries cannot sit at one span");
    assert!(matches!(error, BoundaryError::DuplicateBoundary { .. }));
}

#[test]
fn a_boundary_annotated_outside_the_session_is_refused() {
    let annotated = reference(vec![Boundary::new(99, BoundaryKind::EvidenceAcquisition)]);
    assert!(matches!(
        annotated.check_within(&session(&[1, 2, 3])),
        Err(BoundaryError::BoundaryOutsideSession { seq: 99, .. })
    ));
}

#[test]
fn a_proposal_within_tolerance_matches_and_one_beyond_it_does_not() {
    let annotated = reference(vec![Boundary::new(10, BoundaryKind::AnalysisSelection)]);

    let near = BoundaryAgreement::measure(
        &annotated,
        &[Boundary::new(11, BoundaryKind::AnalysisSelection)],
        1,
    )
    .expect("measurable");
    assert_eq!(near.matched, 1);
    assert_eq!(near.recall, 1.0);

    let far = BoundaryAgreement::measure(
        &annotated,
        &[Boundary::new(13, BoundaryKind::AnalysisSelection)],
        1,
    )
    .expect("measurable");
    assert_eq!(far.matched, 0);
    assert_eq!(far.recall, 0.0);
    assert_eq!(far.precision, 0.0);
}

#[test]
fn the_tolerance_travels_inside_the_agreement_figure() {
    let annotated = reference(vec![Boundary::new(10, BoundaryKind::AnalysisSelection)]);
    let agreement = BoundaryAgreement::measure(
        &annotated,
        &[Boundary::new(12, BoundaryKind::AnalysisSelection)],
        4,
    )
    .expect("measurable");
    assert_eq!(agreement.tolerance, 4);
    assert!(agreement.headline().contains("tolerance 4"));
    let json = serde_json::to_string(&agreement).expect("serialisable");
    assert!(
        json.contains(r#""tolerance":4"#),
        "a recall figure without its tolerance is not a measurement: {json}"
    );
}

#[test]
fn a_proposal_of_the_wrong_kind_does_not_match_however_close_it_sits() {
    let annotated = reference(vec![Boundary::new(10, BoundaryKind::AnalysisSelection)]);
    let agreement = BoundaryAgreement::measure(
        &annotated,
        &[Boundary::new(10, BoundaryKind::ResourceExpenditure)],
        5,
    )
    .expect("measurable");
    assert_eq!(agreement.matched, 0);
}

#[test]
fn one_proposal_cannot_be_credited_against_two_annotated_boundaries() {
    let annotated = reference(vec![
        Boundary::new(10, BoundaryKind::EvidenceAcquisition),
        Boundary::new(11, BoundaryKind::EvidenceAcquisition),
    ]);
    let agreement = BoundaryAgreement::measure(
        &annotated,
        &[Boundary::new(10, BoundaryKind::EvidenceAcquisition)],
        5,
    )
    .expect("measurable");
    assert_eq!(agreement.matched, 1);
    assert_eq!(agreement.recall, 0.5);
    assert_eq!(agreement.precision, 1.0);
}

#[test]
fn a_missed_irreversible_boundary_makes_an_otherwise_high_score_unpublishable() {
    let annotated = reference(vec![
        Boundary::new(1, BoundaryKind::EvidenceAcquisition),
        Boundary::new(2, BoundaryKind::AnalysisSelection),
        Boundary::new(3, BoundaryKind::ClaimCreationOrRevision),
        Boundary::new(4, BoundaryKind::ResourceExpenditure),
        Boundary::new(5, BoundaryKind::HandoffOrEscalation),
        Boundary::new(6, BoundaryKind::GoalOrObligationChange),
        Boundary::new(7, BoundaryKind::IrreversibleOrHighRegretAction),
    ]);
    let proposed: Vec<Boundary> = annotated
        .boundaries()
        .iter()
        .filter(|boundary| !boundary.kind.is_high_regret())
        .copied()
        .collect();

    let agreement = BoundaryAgreement::measure(&annotated, &proposed, 0).expect("measurable");
    assert_eq!(agreement.matched, 6);
    assert_eq!(agreement.precision, 1.0);
    assert!(
        agreement.f1 > 0.9,
        "the aggregate looks excellent: {}",
        agreement.f1
    );
    assert_eq!(
        agreement.missed_high_regret,
        vec![Boundary::new(
            7,
            BoundaryKind::IrreversibleOrHighRegretAction
        )]
    );
    assert!(
        !agreement.is_publishable(),
        "an F-score above 0.9 must not license a suite that never tests the irreversible decision"
    );
    assert!(agreement
        .headline()
        .contains("the aggregate does not stand"));
}

#[test]
fn a_complete_match_including_the_irreversible_boundary_is_publishable() {
    let annotated = reference(vec![
        Boundary::new(2, BoundaryKind::EvidenceAcquisition),
        Boundary::new(7, BoundaryKind::IrreversibleOrHighRegretAction),
    ]);
    let agreement =
        BoundaryAgreement::measure(&annotated, annotated.boundaries(), 0).expect("measurable");
    assert!(agreement.is_publishable());
    assert_eq!(agreement.recall, 1.0);
    assert!(agreement.missed_high_regret.is_empty());
}

#[test]
fn an_agreement_that_matched_nothing_is_not_publishable_even_with_no_irreversible_boundary() {
    let annotated = reference(vec![Boundary::new(2, BoundaryKind::EvidenceAcquisition)]);
    let agreement = BoundaryAgreement::measure(&annotated, &[], 0).expect("measurable");
    assert_eq!(agreement.matched, 0);
    assert!(!agreement.is_publishable());
}

#[test]
fn recall_is_reported_for_every_kind_that_occurs_in_either_set() {
    let annotated = reference(vec![
        Boundary::new(1, BoundaryKind::EvidenceAcquisition),
        Boundary::new(2, BoundaryKind::EvidenceAcquisition),
        Boundary::new(5, BoundaryKind::HandoffOrEscalation),
    ]);
    let agreement = BoundaryAgreement::measure(
        &annotated,
        &[
            Boundary::new(1, BoundaryKind::EvidenceAcquisition),
            Boundary::new(9, BoundaryKind::ResourceExpenditure),
        ],
        0,
    )
    .expect("measurable");

    let kinds: Vec<BoundaryKind> = agreement.by_kind.iter().map(|entry| entry.kind).collect();
    assert!(kinds.contains(&BoundaryKind::EvidenceAcquisition));
    assert!(kinds.contains(&BoundaryKind::HandoffOrEscalation));
    assert!(
        kinds.contains(&BoundaryKind::ResourceExpenditure),
        "a kind proposed but never annotated is a precision failure and must be visible"
    );
    assert!(
        !kinds.contains(&BoundaryKind::AnalysisSelection),
        "a kind absent from both sets is not reported as a perfect or a failed one"
    );

    let evidence = agreement
        .by_kind
        .iter()
        .find(|entry| entry.kind == BoundaryKind::EvidenceAcquisition)
        .expect("present");
    assert_eq!(evidence.recall, 0.5);
    assert_eq!(evidence.precision, 1.0);
}

#[test]
fn the_agreement_contributes_the_meaningful_boundaries_gate_and_leaves_the_rest() {
    let annotated = reference(vec![Boundary::new(2, BoundaryKind::EvidenceAcquisition)]);
    let agreement =
        BoundaryAgreement::measure(&annotated, annotated.boundaries(), 0).expect("measurable");
    let mut audit = ReleaseAudit::open("suite-1", "factory", Auditor::new("independent-site"))
        .expect("an independent auditor");
    agreement.contribute_to(&mut audit);
    assert!(audit.finish().is_err());
}

#[test]
fn cell_spans_tile_the_session_from_each_boundary_to_the_next() {
    let session = session(&[1, 2, 3, 4, 5, 6]);
    let cells = compile_cell_spans(
        &session,
        &[
            Boundary::new(2, BoundaryKind::EvidenceAcquisition),
            Boundary::new(5, BoundaryKind::AnalysisSelection),
        ],
    )
    .expect("compilable");

    assert_eq!(cells.len(), 2);
    assert_eq!((cells[0].start, cells[0].end), (2, 5));
    assert_eq!((cells[1].start, cells[1].end), (5, 7));
    assert_eq!(cells[0].spans, 3);
    assert_eq!(cells[1].spans, 2);
    assert_eq!(cells[0].prefix_spans, 1);
    assert_eq!(cells[1].prefix_spans, 4);
}

#[test]
fn cell_spans_from_a_complete_session_are_replayable() {
    let session = session(&[1, 2, 3]);
    let cells = compile_cell_spans(
        &session,
        &[Boundary::new(2, BoundaryKind::AnalysisSelection)],
    )
    .expect("compilable");
    assert!(cells.iter().all(|cell| cell.replayable.is_yes()));
}

#[test]
fn cell_spans_from_a_gapped_session_say_they_cannot_be_resumed_from() {
    let session = session(&[1, 4, 5]);
    let cells = compile_cell_spans(
        &session,
        &[Boundary::new(4, BoundaryKind::AnalysisSelection)],
    )
    .expect("compilable");
    assert!(matches!(cells[0].replayable, Replayable::No { .. }));
    assert!(session.require_complete().is_err());
}

#[test]
fn a_session_or_a_boundary_set_that_is_empty_compiles_to_no_cells() {
    assert!(compile_cell_spans(&session(&[1, 2]), &[])
        .expect("compilable")
        .is_empty());
    assert!(compile_cell_spans(
        &CaptureSession::new("empty"),
        &[Boundary::new(1, BoundaryKind::AnalysisSelection)]
    )
    .expect("compilable")
    .is_empty());
}

#[test]
fn the_compression_report_says_nothing_about_whether_the_cells_are_the_right_ones() {
    let session = session(&[1, 2, 3, 4, 5, 6]);
    let one_cell = compile_cell_spans(
        &session,
        &[Boundary::new(1, BoundaryKind::GoalOrObligationChange)],
    )
    .expect("compilable");
    let three_cells = compile_cell_spans(
        &session,
        &[
            Boundary::new(1, BoundaryKind::GoalOrObligationChange),
            Boundary::new(3, BoundaryKind::EvidenceAcquisition),
            Boundary::new(5, BoundaryKind::AnalysisSelection),
        ],
    )
    .expect("compilable");

    let lazy = CompressionReport::of(&session, &one_cell);
    let real = CompressionReport::of(&session, &three_cells);
    assert!(
        lazy.spans_per_cell > real.spans_per_cell,
        "the degenerate compiler wins on compression, which is why compression is not the metric \
         that decides anything"
    );
    assert_eq!(real.cells, 3);
    assert_eq!(real.replayable_cells, 3);
    assert_eq!(real.by_kind.len(), 3);
}
