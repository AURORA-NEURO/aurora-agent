//! Invariants of blueprint 25.04, specimen and aliquot lineage.

use bioprism_bioir::{
    ConsumptionEvent, IdentityAssertion, IdentityConfidence, LeakageRisk, LineageError,
    LineageGraph, LineageIssue, ProcessKind, Quantity, Specimen, SpecimenId, SubjectId,
};
use bioprism_scope::Timestamp;
use std::collections::BTreeSet;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("well-formed timestamp")
}

fn sid(text: &str) -> SpecimenId {
    SpecimenId::parse(text).expect("well-formed specimen id")
}

fn subject(text: &str) -> SubjectId {
    SubjectId::parse(text).expect("well-formed subject id")
}

fn ml(amount: f64) -> Quantity {
    Quantity::new(amount, "mL")
}

/// A tumour block collected from one subject, cut into two sections.
fn block_with_two_sections() -> LineageGraph {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("blk-1"),
            subject("pt-1"),
            ts("2026-03-01T09:00:00Z"),
            "left temporal lobe",
            "FFPE block",
            ml(10.0),
        ))
        .expect("root inserts");
    for name in ["blk-1.s1", "blk-1.s2"] {
        graph
            .insert(Specimen::derived(
                sid(name),
                sid("blk-1"),
                ProcessKind::Split,
                ts("2026-03-02T09:00:00Z"),
                "FFPE block",
                ml(2.0),
            ))
            .expect("section inserts");
    }
    graph
}

#[test]
fn two_aliquots_of_one_block_share_a_material_ancestor() {
    let graph = block_with_two_sections();
    let shared = graph
        .nearest_shared_ancestor(&sid("blk-1.s1"), &sid("blk-1.s2"))
        .expect("walk succeeds");
    assert_eq!(shared, Some(sid("blk-1")));

    let risks = graph
        .leakage_risks(&[sid("blk-1.s1"), sid("blk-1.s2")])
        .expect("walk succeeds");
    assert_eq!(
        risks,
        vec![LeakageRisk::SharedMaterialAncestor {
            left: sid("blk-1.s1"),
            right: sid("blk-1.s2"),
            ancestor: sid("blk-1"),
        }]
    );
}

#[test]
fn a_specimen_is_its_own_ancestor_for_sharing_so_a_block_and_its_section_still_leak() {
    let graph = block_with_two_sections();
    let shared = graph
        .nearest_shared_ancestor(&sid("blk-1"), &sid("blk-1.s1"))
        .expect("walk succeeds");
    assert_eq!(shared, Some(sid("blk-1")));
}

#[test]
fn specimens_from_different_subjects_carry_no_shared_ancestry_or_subject() {
    let mut graph = block_with_two_sections();
    graph
        .insert(Specimen::collected(
            sid("blk-2"),
            subject("pt-2"),
            ts("2026-03-01T09:00:00Z"),
            "right frontal lobe",
            "FFPE block",
            ml(8.0),
        ))
        .expect("second root inserts");
    let risks = graph
        .leakage_risks(&[sid("blk-1.s1"), sid("blk-2")])
        .expect("walk succeeds");
    assert!(risks.is_empty(), "unrelated material must not be flagged");
}

#[test]
fn two_collections_from_one_subject_share_the_subject_but_not_the_material() {
    let mut graph = block_with_two_sections();
    graph
        .insert(Specimen::collected(
            sid("blood-1"),
            subject("pt-1"),
            ts("2026-04-01T09:00:00Z"),
            "antecubital vein",
            "whole blood",
            ml(5.0),
        ))
        .expect("second collection inserts");
    let risks = graph
        .leakage_risks(&[sid("blk-1.s1"), sid("blood-1")])
        .expect("walk succeeds");
    assert_eq!(
        risks,
        vec![LeakageRisk::SharedSourceSubject {
            left: sid("blk-1.s1"),
            right: sid("blood-1"),
            subject: subject("pt-1"),
        }]
    );
}

#[test]
fn a_derivative_drawn_after_its_parent_was_consumed_is_reported() {
    let mut graph = LineageGraph::new();
    graph
        .insert(
            Specimen::collected(
                sid("tube-1"),
                subject("pt-1"),
                ts("2026-03-01T09:00:00Z"),
                "antecubital vein",
                "whole blood",
                ml(10.0),
            )
            .with_consumption(ConsumptionEvent {
                consumed_at: ts("2026-03-01T12:00:00Z"),
                amount: None,
                reason: "destructive extraction".to_string(),
            }),
        )
        .expect("root inserts");
    graph
        .insert(Specimen::derived(
            sid("tube-1.a1"),
            sid("tube-1"),
            ProcessKind::Aliquot,
            ts("2026-03-04T09:00:00Z"),
            "whole blood",
            ml(1.0),
        ))
        .expect("aliquot inserts");

    let issues = graph.validate();
    assert!(issues.iter().any(|issue| matches!(
        issue,
        LineageIssue::DrawnFromConsumedParent { child, parent, .. }
            if child == &sid("tube-1.a1") && parent == &sid("tube-1")
    )));
}

#[test]
fn a_derivative_drawn_from_an_exhausted_parent_names_the_draw_that_broke_it() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("tube-1"),
            subject("pt-1"),
            ts("2026-03-01T09:00:00Z"),
            "antecubital vein",
            "plasma",
            ml(10.0),
        ))
        .expect("root inserts");
    graph
        .insert(Specimen::derived(
            sid("tube-1.a1"),
            sid("tube-1"),
            ProcessKind::Aliquot,
            ts("2026-03-02T09:00:00Z"),
            "plasma",
            ml(8.0),
        ))
        .expect("first aliquot inserts");
    graph
        .insert(Specimen::derived(
            sid("tube-1.a2"),
            sid("tube-1"),
            ProcessKind::Aliquot,
            ts("2026-03-03T09:00:00Z"),
            "plasma",
            ml(5.0),
        ))
        .expect("second aliquot inserts");

    let issues = graph.validate();
    let exhausted: Vec<&LineageIssue> = issues
        .iter()
        .filter(|issue| matches!(issue, LineageIssue::DrawnFromExhaustedParent { .. }))
        .collect();
    assert_eq!(
        exhausted.len(),
        1,
        "only the draw that could not be satisfied is at fault"
    );
    assert!(matches!(
        exhausted[0],
        LineageIssue::DrawnFromExhaustedParent { child, .. } if child == &sid("tube-1.a2")
    ));
    assert!(issues
        .iter()
        .any(|issue| matches!(issue, LineageIssue::MassBalanceExceeded { .. })));
}

#[test]
fn a_cultured_derivative_may_exceed_its_parent_because_cells_divide() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("biopsy-1"),
            subject("pt-1"),
            ts("2026-03-01T09:00:00Z"),
            "tumour core",
            "fresh tissue",
            ml(1.0),
        ))
        .expect("root inserts");
    graph
        .insert(Specimen::derived(
            sid("biopsy-1.c1"),
            sid("biopsy-1"),
            ProcessKind::Culture,
            ts("2026-03-20T09:00:00Z"),
            "fresh tissue",
            ml(50.0),
        ))
        .expect("culture inserts");

    assert!(
        graph.validate().is_empty(),
        "expansion in culture is not a mass-balance violation"
    );
    assert_eq!(graph.drawn_from(&sid("biopsy-1")).unwrap().amount, 0.0);
}

#[test]
fn consent_labels_cannot_be_widened_by_splitting_material() {
    let mut graph = LineageGraph::new();
    graph
        .insert(
            Specimen::collected(
                sid("blk-1"),
                subject("pt-1"),
                ts("2026-03-01T09:00:00Z"),
                "left temporal lobe",
                "FFPE block",
                ml(10.0),
            )
            .with_consent(["research-use"]),
        )
        .expect("root inserts");
    graph
        .insert(
            Specimen::derived(
                sid("blk-1.s1"),
                sid("blk-1"),
                ProcessKind::Split,
                ts("2026-03-02T09:00:00Z"),
                "FFPE block",
                ml(2.0),
            )
            .with_consent(["research-use", "commercial-use"]),
        )
        .expect("section inserts");

    let issues = graph.validate();
    assert!(issues.contains(&LineageIssue::ConsentExpanded {
        child: sid("blk-1.s1"),
        parent: sid("blk-1"),
        label: "commercial-use".to_string(),
    }));
}

#[test]
fn a_disputed_identity_is_surfaced_rather_than_resolved() {
    let mut graph = block_with_two_sections();
    let mut conflicting = BTreeSet::new();
    conflicting.insert(subject("pt-1"));
    conflicting.insert(subject("pt-9"));
    let disputed = Specimen::derived(
        sid("blk-1.s3"),
        sid("blk-1"),
        ProcessKind::Split,
        ts("2026-03-02T09:00:00Z"),
        "FFPE block",
        ml(1.0),
    )
    .with_identity(IdentityAssertion {
        asserted_subject: subject("pt-1"),
        confidence: IdentityConfidence::Disputed { conflicting },
        evidence: vec!["genotype concordance 0.61".to_string()],
    });
    graph.insert(disputed).expect("section inserts");

    let issues = graph.validate();
    assert!(issues.contains(&LineageIssue::DisputedIdentity {
        specimen: sid("blk-1.s3"),
        count: 2,
    }));
    assert_eq!(
        graph.source_subject(&sid("blk-1.s3")).unwrap(),
        subject("pt-1"),
        "the dispute is reported without the query refusing to answer"
    );
}

#[test]
fn a_local_identity_assertion_contradicting_the_collection_record_is_reported() {
    let mut graph = block_with_two_sections();
    let relabelled = Specimen::derived(
        sid("blk-1.s4"),
        sid("blk-1"),
        ProcessKind::Split,
        ts("2026-03-02T09:00:00Z"),
        "FFPE block",
        ml(1.0),
    )
    .with_identity(IdentityAssertion {
        asserted_subject: subject("pt-7"),
        confidence: IdentityConfidence::Verified {
            method: "STR profiling".to_string(),
        },
        evidence: vec![],
    });
    graph.insert(relabelled).expect("section inserts");

    let issues = graph.validate();
    assert!(issues.contains(&LineageIssue::IdentityConflict {
        specimen: sid("blk-1.s4"),
        asserted: subject("pt-7"),
        inherited: subject("pt-1"),
    }));
    assert_eq!(
        graph.source_subject(&sid("blk-1.s4")).unwrap(),
        subject("pt-7"),
        "an independent verification overrides the paperwork"
    );
}

#[test]
fn a_cyclic_lineage_is_refused_rather_than_walked_forever() {
    let mut graph = LineageGraph::new();
    for (child, parent) in [("a", "b"), ("b", "a")] {
        graph
            .insert(Specimen::derived(
                sid(child),
                sid(parent),
                ProcessKind::Split,
                ts("2026-03-02T09:00:00Z"),
                "FFPE block",
                ml(1.0),
            ))
            .expect("inserts");
    }
    assert!(matches!(
        graph.ancestors(&sid("a")),
        Err(LineageError::Cycle { .. })
    ));
    assert!(graph
        .validate()
        .iter()
        .any(|issue| matches!(issue, LineageIssue::Cycle { .. })));
}

#[test]
fn quantities_in_different_units_are_never_silently_converted() {
    let error = ml(1.0)
        .add(&Quantity::new(500.0, "uL"), "blk-1.s1")
        .expect_err("units differ");
    assert_eq!(
        error,
        LineageError::UnitMismatch {
            subject: "blk-1.s1".to_string(),
            left: "mL".to_string(),
            right: "uL".to_string(),
        }
    );

    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("tube-1"),
            subject("pt-1"),
            ts("2026-03-01T09:00:00Z"),
            "antecubital vein",
            "plasma",
            ml(10.0),
        ))
        .expect("root inserts");
    graph
        .insert(Specimen::derived(
            sid("tube-1.a1"),
            sid("tube-1"),
            ProcessKind::Aliquot,
            ts("2026-03-02T09:00:00Z"),
            "plasma",
            Quantity::new(500.0, "uL"),
        ))
        .expect("aliquot inserts");
    assert!(graph
        .validate()
        .iter()
        .any(|issue| matches!(issue, LineageIssue::UnitMismatch { .. })));
}

#[test]
fn a_third_generation_aliquot_still_resolves_to_its_collection_event() {
    let mut graph = block_with_two_sections();
    graph
        .insert(Specimen::derived(
            sid("blk-1.s1.curl"),
            sid("blk-1.s1"),
            ProcessKind::Extraction {
                analyte: "genomic DNA".to_string(),
            },
            ts("2026-03-05T09:00:00Z"),
            "FFPE block",
            ml(0.5),
        ))
        .expect("curl inserts");

    assert_eq!(
        graph.ancestors(&sid("blk-1.s1.curl")).unwrap(),
        vec![sid("blk-1.s1"), sid("blk-1")]
    );
    assert_eq!(
        graph.collection(&sid("blk-1.s1.curl")).unwrap().site,
        "left temporal lobe"
    );
    assert_eq!(
        graph.source_subject(&sid("blk-1.s1.curl")).unwrap(),
        subject("pt-1")
    );
    assert_eq!(
        graph.descendants(&sid("blk-1")).unwrap().len(),
        3,
        "both sections and the curl descend from the block"
    );
}

#[test]
fn a_duplicate_specimen_identifier_is_refused() {
    let mut graph = block_with_two_sections();
    let error = graph
        .insert(Specimen::collected(
            sid("blk-1"),
            subject("pt-2"),
            ts("2026-03-01T09:00:00Z"),
            "right frontal lobe",
            "FFPE block",
            ml(4.0),
        ))
        .expect_err("the identifier is taken");
    assert_eq!(
        error,
        LineageError::DuplicateSpecimen {
            specimen: "blk-1".to_string()
        }
    );
}

#[test]
fn remaining_quantity_accounts_for_every_draw_and_the_consumption() {
    let graph = block_with_two_sections();
    let mut block = graph.get(&sid("blk-1")).unwrap().clone();
    block.consumption = Some(ConsumptionEvent {
        consumed_at: ts("2026-03-10T09:00:00Z"),
        amount: Some(ml(1.0)),
        reason: "diagnostic section".to_string(),
    });
    let mut with_consumption = LineageGraph::new();
    with_consumption.insert(block).expect("root inserts");
    for name in ["blk-1.s1", "blk-1.s2"] {
        with_consumption
            .insert(graph.get(&sid(name)).unwrap().clone())
            .expect("section inserts");
    }

    assert_eq!(with_consumption.drawn_from(&sid("blk-1")).unwrap().amount, 4.0);
    assert_eq!(with_consumption.remaining(&sid("blk-1")).unwrap().amount, 5.0);
    assert!(with_consumption.validate().is_empty());
}

#[test]
fn a_derivative_drawn_before_its_parent_existed_is_reported() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("blk-1"),
            subject("pt-1"),
            ts("2026-03-01T09:00:00Z"),
            "left temporal lobe",
            "FFPE block",
            ml(10.0),
        ))
        .expect("root inserts");
    graph
        .insert(Specimen::derived(
            sid("blk-1.s1"),
            sid("blk-1"),
            ProcessKind::Split,
            ts("2026-02-01T09:00:00Z"),
            "FFPE block",
            ml(2.0),
        ))
        .expect("section inserts");

    assert!(graph
        .validate()
        .iter()
        .any(|issue| matches!(issue, LineageIssue::DrawnBeforeParentExisted { .. })));
}

#[test]
fn a_derivative_naming_a_parent_that_is_not_in_the_graph_is_reported() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::derived(
            sid("orphan"),
            sid("never-collected"),
            ProcessKind::Aliquot,
            ts("2026-03-02T09:00:00Z"),
            "plasma",
            ml(1.0),
        ))
        .expect("inserts");
    assert!(graph.validate().contains(&LineageIssue::UnknownParent {
        child: sid("orphan"),
        parent: sid("never-collected"),
    }));
}
