//! Invariant tests for the projection layer.
//!
//! Each test name states the claim it defends. The claims come from blueprint 43.01 (projections
//! are generated, provenance-bearing and never a completeness proof), 41.03 (the normative edge
//! vocabulary), 43.09 (event time is not availability time) and 40.29 (the table fallback).

use bioprism_graph::{
    evidence_survives, lint_graph, obstructions_survive, project_all, Availability, BoundSection,
    ClockAnomaly, DropReason, EdgeType, FidelityLedger, GraphBody, GraphEdge, GraphLint, GraphNode,
    GraphProjection, HypergraphProjection, NodeKind, NodeStatus, OrderJustification, ProjectRegion,
    Projection, ProjectionError, ProjectionKind, ProjectionSource, TableProjection, TimelineAxis,
    TimelineProjection, COLUMNS, RENDERING_NOTE,
};
use bioprism_section::{
    Backend, CertificateProfile, ContextCertificate, DecisionSection, EvidenceCapsule,
    LeakageWitness, OmissionManifest, OracleVerdict, PlanDescriptor, ReferenceOmissions,
    RefinementOption, SourceHashes, UnresolvedObligation,
};
use bioprism_world::{CausalEvent, World};
use bioprism_worldgen::WorldSpec;
use serde_json::{json, Value};

fn capsule(id: &str, provides: &str, value: Value, provenance: &[&str]) -> EvidenceCapsule {
    EvidenceCapsule::from_raw_fact(&json!({
        "id": id,
        "provides": provides,
        "value": value,
        "scope": { "cohort": "C-01" },
        "tags": ["identity"],
        "provenance": provenance,
    }))
}

/// A blocked, contradicted region: two unresolved obligations, one oracle witness, one multiway
/// factor, and one evidence capsule that no selected factor consumes.
fn section() -> DecisionSection {
    DecisionSection {
        world_id: "world.radiogenomic".into(),
        query_id: "query.split_integrity".into(),
        decision_time: "2025-02-15T00:00:00Z".into(),
        goal: "decide whether the cohort split is valid".into(),
        selected_evidence: vec![
            capsule(
                "fact.split_assignment",
                "split_assignment",
                json!({ "train": ["S001"], "test": ["S002"] }),
                &["doc://split.csv#L1"],
            ),
            capsule("fact.cohort_id", "cohort_id", json!("C-01"), &[]),
            capsule(
                "fact.orphan_note",
                "orphan_note",
                json!("no selected factor consumes this"),
                &[],
            ),
        ],
        selected_factors: vec![
            json!({
                "id": "factor.identity_check",
                "kind": "deterministic_rule",
                "inputs": ["cohort_id", "subject_aliases", "split_assignment"],
                "outputs": ["identity_leakage"],
                "scope": { "cohort": "C-01" }
            }),
            json!({
                "id": "factor.policy_check",
                "kind": "deterministic_rule",
                "inputs": ["data_policy"],
                "outputs": ["policy_validity"]
            }),
        ],
        oracle: OracleVerdict::new(
            "deterministic_split_integrity_v1",
            vec![LeakageWitness::PreprocessingLeakage {
                detail: "preprocessing fit used all subjects before the split was drawn".into(),
            }],
        ),
        unresolved_obligations: vec![
            UnresolvedObligation::InaccessibleAtCut {
                fact_id: "fact.future_label".into(),
            },
            UnresolvedObligation::PolicyBlocked {
                detail: "consent withheld for site B".into(),
            },
        ],
        refinement_frontier: vec![RefinementOption {
            action: "advance_time_cut_or_use_retrospective_mode".into(),
            facts: vec!["fact.future_label".into()],
        }],
    }
}

/// A certificate that genuinely attests `section`.
fn certificate_for(section: &DecisionSection) -> ContextCertificate {
    ContextCertificate {
        world_id: section.world_id.clone(),
        query_id: section.query_id.clone(),
        selected_facts: section
            .evidence_ids()
            .iter()
            .map(|id| id.to_string())
            .collect(),
        selected_factors: vec!["factor.identity_check".into(), "factor.policy_check".into()],
        protected_closure: vec!["fact.split_assignment".into()],
        omissions: ReferenceOmissions {
            total_facts: 12,
            exploratory_facts: 9,
            classification: "no_backward_dependency_path_or_temporally_inaccessible".into(),
            inaccessible_selected_before_cut: vec!["fact.future_label".into()],
        },
        plan: PlanDescriptor {
            backend: Backend::BackwardFactorSliceReference,
            compiled_factor_count: 2,
            compiled_fact_count: 3,
            total_factor_count: 8,
            total_fact_count: 12,
            max_selected_factor_arity: 3,
            fallback: None,
        },
        oracle: section.oracle.clone(),
        source_hashes: SourceHashes {
            world_sha256: "00".repeat(32),
            query_sha256: "11".repeat(32),
            decision_section_sha256: section
                .content_hash()
                .expect("section digests")
                .as_str()
                .to_string(),
        },
        limitations: vec!["reference slicer".into()],
        manifest: OmissionManifest::default(),
    }
}

fn bound() -> (DecisionSection, ContextCertificate, ProjectionSource) {
    let section = section();
    let certificate = certificate_for(&section);
    let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Extended)
        .expect("certificate attests this section");
    (section, certificate, source)
}

fn event(
    id: &str,
    event_time: &str,
    availability_time: &str,
    produces: &[&str],
    parents: &[&str],
) -> CausalEvent {
    CausalEvent::from_json(&json!({
        "id": id,
        "event_time": event_time,
        "availability_time": availability_time,
        "produces": produces,
        "causal_parents": parents,
    }))
    .expect("event fixture parses")
}

/// Two roots that no causal relation orders, one released after the cut, one backdated.
fn events() -> Vec<CausalEvent> {
    vec![
        event(
            "event.training",
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
            &["split_assignment"],
            &[],
        ),
        event(
            "event.scan",
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
            &["scan_series"],
            &[],
        ),
        event(
            "event.future_label",
            "2025-06-01T00:00:00Z",
            "2025-06-15T00:00:00Z",
            &["future_label_value"],
            &["event.training"],
        ),
        event(
            "event.amended_report",
            "2025-07-01T00:00:00Z",
            "2025-02-01T00:00:00Z",
            &["report_text"],
            &["event.scan"],
        ),
    ]
}

#[test]
fn a_projection_cannot_be_constructed_without_the_certificate_it_came_from() {
    // The only route to a `View` outside this crate is `Projection::project`, which demands a
    // `ProjectionSource`; the only constructor for a `ProjectionSource` is `bind`, which demands
    // both the section and the certificate and hashes them itself. There is no path that produces
    // a view from a section alone, which is why this test asserts on what `bind` produces rather
    // than on a compile failure.
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    assert_eq!(view.source().query_id(), section.query_id);
    assert_eq!(view.source().world_id(), section.world_id);
    assert_eq!(view.source().decision_time(), section.decision_time);
    assert_eq!(
        view.source().section_sha256(),
        section.content_hash().unwrap().as_str()
    );
    assert_eq!(view.source().certificate_sha256().len(), 64);
    assert_eq!(view.source().certificate_profile(), "extended");
}

#[test]
fn a_view_cannot_be_bound_to_a_certificate_that_attests_a_different_section() {
    let section = section();
    let mut other = section.clone();
    other.goal = "a different decision entirely".into();
    let certificate = certificate_for(&other);

    let error = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
        .expect_err("the certificate attests another section");
    assert!(matches!(
        error,
        ProjectionError::CertificateAttestsAnotherSection { .. }
    ));
}

#[test]
fn binding_refuses_a_certificate_issued_for_another_world() {
    let section = section();
    let mut certificate = certificate_for(&section);
    certificate.world_id = "world.somewhere_else".into();

    let error = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
        .expect_err("world ids disagree");
    match error {
        ProjectionError::IdentityMismatch { field, .. } => assert_eq!(field, "world_id"),
        other => panic!("expected an identity mismatch, got {other}"),
    }
}

#[test]
fn a_view_refuses_to_render_when_the_section_changed_after_provenance_was_bound() {
    let (mut section, _certificate, source) = bound();
    section.goal = "quietly repurposed after the certificate was issued".into();

    let error = GraphProjection::new()
        .project(&section, source)
        .expect_err("the section drifted");
    assert!(matches!(
        error,
        ProjectionError::SectionMutatedAfterBinding { .. }
    ));
}

#[test]
fn verifying_a_view_against_a_different_section_reports_a_digest_mismatch() {
    let (section, certificate, source) = bound();
    let view = TableProjection::new()
        .project(&section, source)
        .expect("projects");

    assert!(view
        .verify(&section, &certificate, CertificateProfile::Extended)
        .unwrap()
        .is_match());

    let mut tampered = section.clone();
    tampered.decision_time = "2099-01-01T00:00:00Z".into();
    assert!(!view
        .verify(&tampered, &certificate, CertificateProfile::Extended)
        .unwrap()
        .is_match());
}

#[test]
fn verifying_a_view_under_the_wrong_certificate_profile_reports_a_digest_mismatch() {
    let (section, certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    // The extended profile hashes a strictly larger body, so the same certificate read under the
    // reference profile digests differently. A consumer must not be able to swap profiles silently.
    assert!(!view
        .verify(&section, &certificate, CertificateProfile::Reference)
        .unwrap()
        .is_match());
}

/// A projection that renders the right body but forgets to declare what it carried.
struct ForgetfulProjection;

impl Projection for ForgetfulProjection {
    type Body = GraphBody;
    const KIND: ProjectionKind = ProjectionKind::Graph;

    fn render(
        &self,
        section: &DecisionSection,
        _ledger: &mut FidelityLedger,
    ) -> Result<GraphBody, ProjectionError> {
        GraphProjection::new().render(section, &mut FidelityLedger::default())
    }
}

#[test]
fn a_projection_that_fails_to_carry_an_unresolved_obligation_cannot_seal_a_view() {
    let (section, _certificate, source) = bound();
    let error = ForgetfulProjection
        .project(&section, source)
        .expect_err("the ledger refuses to close");
    match error {
        ProjectionError::ObligationDropped {
            expected, carried, ..
        } => {
            assert_eq!(expected, 2);
            assert_eq!(carried, 0);
        }
        other => panic!("expected a dropped obligation, got {other}"),
    }
}

/// Carries the obligations but not the witness, isolating the conflict guard.
struct ObligationOnlyProjection;

impl Projection for ObligationOnlyProjection {
    type Body = GraphBody;
    const KIND: ProjectionKind = ProjectionKind::Graph;

    fn render(
        &self,
        section: &DecisionSection,
        ledger: &mut FidelityLedger,
    ) -> Result<GraphBody, ProjectionError> {
        let mut inner = FidelityLedger::default();
        let body = GraphProjection::new().render(section, &mut inner)?;
        for id in &body.obligation_nodes {
            ledger.carry_obligation(id.clone());
        }
        Ok(body)
    }
}

#[test]
fn a_projection_that_fails_to_carry_an_oracle_conflict_cannot_seal_a_view() {
    let (section, _certificate, source) = bound();
    let error = ObligationOnlyProjection
        .project(&section, source)
        .expect_err("the ledger refuses to close");
    match error {
        ProjectionError::ConflictDropped {
            expected, carried, ..
        } => {
            assert_eq!(expected, 1);
            assert_eq!(carried, 0);
        }
        other => panic!("expected a dropped conflict, got {other}"),
    }
}

#[test]
fn unresolved_obligations_and_conflicts_survive_into_every_projection() {
    let (section, _certificate, source) = bound();
    let bundle = project_all(&section, &events(), source).expect("projects four ways");

    assert!(bundle.obstructions_survive_everywhere(&section));
    for coverage in [
        obstructions_survive(&section, &bundle.graph),
        obstructions_survive(&section, &bundle.hypergraph),
        obstructions_survive(&section, &bundle.timeline),
        obstructions_survive(&section, &bundle.table),
    ] {
        assert!(coverage.is_complete(), "missing {:?}", coverage.missing);
        assert_eq!(coverage.recovered.len(), 3);
    }
}

#[test]
fn evidence_handles_survive_the_graph_hypergraph_and_table_round_trip() {
    let (section, _certificate, source) = bound();
    let bundle = project_all(&section, &events(), source).expect("projects four ways");

    assert!(evidence_survives(&section, &bundle.graph).is_complete());
    assert!(evidence_survives(&section, &bundle.hypergraph).is_complete());
    assert!(evidence_survives(&section, &bundle.table).is_complete());

    // The timeline's subject is events, not evidence. It legitimately cannot resolve an evidence
    // id, and its loss ledger says so rather than the view implying coverage it does not have.
    let timeline = evidence_survives(&section, &bundle.timeline);
    assert!(!timeline.is_complete());
    assert!(
        bundle
            .timeline
            .fidelity()
            .dropped_for(DropReason::ValuesElided)
            .count()
            > 0
    );
}

#[test]
fn the_edge_vocabulary_reproduces_the_normative_glosses_verbatim() {
    let expected = [
        (EdgeType::Contains, "source indexes target"),
        (EdgeType::Evaluates, "source tests target"),
        (EdgeType::Governs, "target constrains source"),
        (EdgeType::Implements, "source realizes target"),
        (EdgeType::PartOf, "source belongs to target section"),
        (EdgeType::Provides, "source produces inputs used by target"),
        (EdgeType::References, "source links to target"),
        (EdgeType::Related, "non-normative adjacency"),
        (EdgeType::Requires, "source depends on target contract"),
        (EdgeType::Supersedes, "source replaces target"),
    ];
    for (edge, gloss) in expected {
        assert_eq!(edge.gloss(), gloss, "gloss drifted for {edge}");
    }
    assert_eq!(EdgeType::ALL.len(), 10);
    assert_eq!(
        EdgeType::ALL
            .into_iter()
            .filter(|e| !e.is_normative())
            .count(),
        1
    );
}

#[test]
fn unknown_edge_types_fail_validation() {
    assert_eq!(EdgeType::parse("part_of").unwrap(), EdgeType::PartOf);
    let error = EdgeType::parse("sort_of_related_to").expect_err("not in the vocabulary");
    assert!(matches!(error, ProjectionError::UnknownEdgeType { .. }));
}

#[test]
fn the_graph_projection_never_emits_a_guessed_related_or_supersedes_edge() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    assert_eq!(body.edges_of(EdgeType::Related).count(), 0);
    assert_eq!(body.edges_of(EdgeType::Supersedes).count(), 0);
    assert!(body.edges.iter().all(|edge| edge.edge.is_normative()));

    // Absence is stated in the payload with its reason, not left for a reader to notice.
    let absent: Vec<EdgeType> = body.not_emitted.iter().map(|note| note.edge).collect();
    assert!(absent.contains(&EdgeType::Related));
    assert!(absent.contains(&EdgeType::Supersedes));
    assert!(body.not_emitted.iter().all(|note| !note.reason.is_empty()));
}

#[test]
fn the_governs_edge_runs_from_the_decision_to_the_obligation_that_constrains_it() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    let governs: Vec<&GraphEdge> = body.edges_of(EdgeType::Governs).collect();
    assert_eq!(governs.len(), 2);
    for edge in governs {
        assert_eq!(edge.from, body.decision_node);
        assert_eq!(
            body.node(&edge.to).map(|node| node.kind),
            Some(NodeKind::Obligation),
            "the gloss is \"target constrains source\", so the obligation must be the target"
        );
    }
}

#[test]
fn a_fact_named_by_an_obligation_appears_as_a_withheld_node_rather_than_being_omitted() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    let withheld = view
        .body()
        .node("fact.future_label")
        .expect("the hole is rendered, not closed");
    assert_eq!(withheld.status, NodeStatus::Withheld);
    assert_eq!(withheld.handle, "fact.future_label");
}

#[test]
fn an_evidence_capsule_no_factor_consumes_is_still_projected() {
    // 43.01: completeness is defined against query obligations, not neighbourhood radius. A
    // capsule that no selected factor reads is zero-degree under a `provides` traversal, and any
    // radius- or hop-based view would drop it. The compiler put it in the region; the view keeps
    // it.
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    let orphan = body.node("fact.orphan_note").expect("kept");
    assert_eq!(orphan.kind, NodeKind::Evidence);
    assert_eq!(orphan.status, NodeStatus::Delivered);
    assert_eq!(
        body.outgoing("fact.orphan_note")
            .filter(|edge| edge.edge == EdgeType::Provides)
            .count(),
        0,
        "nothing consumes it, which is exactly why a radius rule would have lost it"
    );
}

#[test]
fn every_evidence_node_keeps_the_scope_its_value_is_only_valid_in() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    for node in view.body().nodes_of(NodeKind::Evidence) {
        if node.status == NodeStatus::Withheld {
            continue;
        }
        assert!(
            node.scope.is_some(),
            "{} lost its validity scope, turning a local section into a global claim",
            node.id
        );
    }
}

#[test]
fn the_graph_projection_has_no_dangling_edge_endpoints_and_no_requires_cycle() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    let findings = lint_graph(view.body());
    assert!(
        !findings.iter().any(|finding| matches!(
            finding,
            GraphLint::DanglingEndpoint { .. } | GraphLint::RequiresCycle { .. }
        )),
        "{findings:?}"
    );
}

#[test]
fn a_requires_cycle_in_an_assembled_view_is_linted_rather_than_hidden() {
    // 41.03: "requires edges are acyclic at the release-contract layer". This crate's own
    // projection cannot produce a cycle — `requires` only ever runs factor to variable — so the
    // lint is exercised against a view assembled elsewhere and handed here for checking.
    let node = |id: &str| GraphNode {
        id: id.into(),
        kind: NodeKind::Factor,
        label: "assembled elsewhere".into(),
        status: NodeStatus::Delivered,
        handle: id.into(),
        scope: None,
    };
    let edge = |from: &str, to: &str| GraphEdge {
        from: from.into(),
        edge: EdgeType::Requires,
        to: to.into(),
    };
    let body = GraphBody {
        decision_node: "decision:q".into(),
        nodes: vec![node("a"), node("b")],
        edges: vec![edge("a", "b"), edge("b", "a")],
        obligation_nodes: vec![],
        conflict_nodes: vec![],
        multiway_factors: vec![],
        edge_vocabulary: vec![],
        not_emitted: vec![],
    };

    let findings = lint_graph(&body);
    match findings
        .iter()
        .find(|f| matches!(f, GraphLint::RequiresCycle { .. }))
    {
        Some(GraphLint::RequiresCycle { members }) => {
            assert_eq!(members, &vec!["a".to_string(), "b".to_string()]);
        }
        other => panic!("expected a requires cycle, got {other:?}"),
    }
}

#[test]
fn the_graph_projection_reports_the_multiway_factors_it_had_to_flatten() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    let notes = &view.body().multiway_factors;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].factor_id, "factor.identity_check");
    assert_eq!(notes[0].arity, 4);
    assert_eq!(
        notes[0].inspector,
        ProjectionKind::Hypergraph,
        "43.01 requires naming the inspector, not just warning"
    );
    assert!(view
        .fidelity()
        .dropped_for(DropReason::FlattenedToBinaryEdges)
        .any(|dropped| dropped.recover_from.contains("hypergraph")));
    assert!(view.fidelity().has_semantic_loss());
}

#[test]
fn the_hypergraph_keeps_whole_the_factor_the_graph_had_to_split() {
    let (section, _certificate, source) = bound();
    let view = HypergraphProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    let edge = body.edge("factor.identity_check").expect("kept whole");
    assert_eq!(edge.arity, 4);
    assert!(edge.multiway);
    assert_eq!(edge.pins.len(), 4);
    assert_eq!(body.max_arity, 4);
    assert_eq!(body.multiway_edges().count(), 1);

    // A vertex knows which evidence supplies it, so the inspector is navigable in both directions.
    let split = body
        .vertices
        .iter()
        .find(|vertex| vertex.variable == "split_assignment")
        .expect("present");
    assert_eq!(split.supplied_by, vec!["fact.split_assignment".to_string()]);
    assert_eq!(
        split.incident_edges,
        vec!["factor.identity_check".to_string()]
    );
}

#[test]
fn the_hypergraph_declares_inside_its_payload_that_explicit_incidence_is_a_rendering_choice() {
    let (section, _certificate, source) = bound();
    let view = HypergraphProjection::new()
        .project(&section, source)
        .expect("projects");

    let note = &view.body().rendering_note;
    assert_eq!(note, RENDERING_NOTE);
    assert!(note.contains("generated projection"));
    assert!(note.contains("not an incidence list"));
    assert!(note.contains("read back as storage or as execution semantics"));

    // The note travels with the serialised view, not only in the crate docs.
    let wire = serde_json::to_value(&view).expect("serialises");
    assert!(wire["body"]["rendering_note"]
        .as_str()
        .expect("string")
        .contains("43.01"));
}

#[test]
fn the_timeline_never_merges_event_time_with_availability_time() {
    let (section, _certificate, source) = bound();
    let view = TimelineProjection::new(&events())
        .project(&section, source)
        .expect("projects");

    let future = view.body().entry("event.future_label").expect("rendered");
    assert_eq!(future.event_time, "2025-06-01T00:00:00Z");
    assert_eq!(future.availability_time, "2025-06-15T00:00:00Z");
    assert_ne!(future.event_time, future.availability_time);
}

#[test]
fn an_event_released_after_the_cut_is_marked_withheld_rather_than_dropped() {
    let (section, _certificate, source) = bound();
    let view = TimelineProjection::new(&events())
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    assert_eq!(body.decision_cut, "2025-02-15T00:00:00Z");
    let withheld: Vec<&str> = body
        .withheld_at_cut()
        .map(|entry| entry.event_id.as_str())
        .collect();
    assert_eq!(withheld, vec!["event.future_label"]);
    assert_eq!(
        body.entry("event.training").unwrap().availability,
        Availability::AvailableAtCut
    );
    assert_eq!(body.entries.len(), 4, "nothing was filtered out");
}

#[test]
fn a_backdated_event_is_reported_as_a_clock_anomaly_and_not_corrected() {
    let (section, _certificate, source) = bound();
    let view = TimelineProjection::new(&events())
        .project(&section, source)
        .expect("projects");

    let amended = view.body().entry("event.amended_report").expect("rendered");
    assert_eq!(
        amended.anomaly,
        Some(ClockAnomaly::AvailableBeforeItHappened)
    );
    assert_eq!(amended.event_time, "2025-07-01T00:00:00Z");
    assert_eq!(amended.availability_time, "2025-02-01T00:00:00Z");
}

#[test]
fn the_timeline_declares_every_adjacency_it_imposed_on_concurrent_events() {
    let (section, _certificate, source) = bound();
    let view = TimelineProjection::new(&events())
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    // `event.training` and `event.scan` share an availability time and no causal relation, so the
    // line orders them and must say that it did.
    assert!(body.imposed_adjacencies() > 0);
    let scan = body.entry("event.scan").expect("rendered");
    assert!(scan.concurrent_with.contains(&"event.training".to_string()));

    assert!(view
        .fidelity()
        .dropped_for(DropReason::TotallyOrderedForDisplay)
        .any(|dropped| dropped.count == body.imposed_adjacencies()));
}

#[test]
fn changing_the_timeline_axis_changes_the_order_and_not_the_membership() {
    let (section, _certificate, source) = bound();
    let by_availability = TimelineProjection::on_axis(&events(), TimelineAxis::AvailabilityTime)
        .project(&section, source.clone())
        .expect("projects");
    let by_event_time = TimelineProjection::on_axis(&events(), TimelineAxis::EventTime)
        .project(&section, source)
        .expect("projects");

    let ids = |body: &bioprism_graph::TimelineBody| {
        let mut names: Vec<String> = body.entries.iter().map(|e| e.event_id.clone()).collect();
        names.sort();
        names
    };
    assert_eq!(ids(by_availability.body()), ids(by_event_time.body()));
    assert_eq!(by_availability.body().axis, TimelineAxis::AvailabilityTime);
    assert_eq!(by_event_time.body().axis, TimelineAxis::EventTime);

    let first = |body: &bioprism_graph::TimelineBody| body.entries[0].event_id.clone();
    // The backdated report is released before it happens, so the two axes disagree about where it
    // sits — which is the whole reason both axes exist.
    assert_ne!(
        by_availability
            .body()
            .entries
            .iter()
            .position(|e| e.event_id == "event.amended_report"),
        by_event_time
            .body()
            .entries
            .iter()
            .position(|e| e.event_id == "event.amended_report"),
    );
    assert_eq!(first(by_availability.body()), "event.scan");
}

#[test]
fn a_causal_cycle_is_named_rather_than_silently_ordered() {
    let (section, _certificate, source) = bound();
    let cyclic = vec![
        event(
            "event.a",
            "2025-01-01T00:00:00Z",
            "2025-01-01T00:00:00Z",
            &[],
            &["event.b"],
        ),
        event(
            "event.b",
            "2025-01-02T00:00:00Z",
            "2025-01-02T00:00:00Z",
            &[],
            &["event.a"],
        ),
    ];
    let view = TimelineProjection::new(&cyclic)
        .project(&section, source)
        .expect("projects");

    assert_eq!(
        view.body().causal_cycle_members,
        vec!["event.a".to_string(), "event.b".to_string()]
    );
}

#[test]
fn the_timeline_renders_events_that_produced_nothing_the_region_selected() {
    let (section, _certificate, source) = bound();
    let view = TimelineProjection::new(&events())
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    let scan = body.entry("event.scan").expect("rendered anyway");
    assert!(scan.produces_selected.is_empty());
    let training = body.entry("event.training").expect("rendered");
    assert_eq!(
        training.produces_selected,
        vec!["split_assignment".to_string()]
    );
}

#[test]
fn an_unreadable_decision_cut_is_a_typed_error_rather_than_a_guessed_instant() {
    let mut section = section();
    section.decision_time = "sometime last spring".into();
    let certificate = certificate_for(&section);
    let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
        .expect("binds");

    let error = TimelineProjection::new(&events())
        .project(&section, source)
        .expect_err("no cut, no availability verdict");
    assert!(matches!(
        error,
        ProjectionError::UnreadableDecisionTime { .. }
    ));
}

#[test]
fn the_table_lists_obligations_and_conflicts_before_any_evidence_row() {
    let (section, _certificate, source) = bound();
    let view = TableProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    assert_eq!(body.obstruction_rows, 3);
    for row in &body.rows[..body.obstruction_rows] {
        assert!(matches!(
            row.kind,
            NodeKind::Obligation | NodeKind::Conflict
        ));
    }
    let first_evidence = body
        .rows
        .iter()
        .position(|row| row.kind == NodeKind::Evidence)
        .expect("evidence rows exist");
    assert!(first_evidence >= body.obstruction_rows);
}

#[test]
fn the_table_publishes_its_column_header_and_emits_a_cell_for_each_column() {
    let (section, _certificate, source) = bound();
    let view = TableProjection::new()
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    assert_eq!(body.columns, COLUMNS.map(String::from).to_vec());
    for row in &body.rows {
        assert_eq!(row.cells().len(), COLUMNS.len());
    }
    assert!(body.caption.contains("obstruction row(s) first"));
}

#[test]
fn the_table_carries_evidence_values_in_full_without_truncation() {
    let (section, _certificate, source) = bound();
    let view = TableProjection::new()
        .project(&section, source)
        .expect("projects");

    let row = view.body().row("fact.split_assignment").expect("present");
    assert!(row.detail.contains("S001"));
    assert!(row.detail.contains("S002"));
    assert!(row.detail.contains("cohort"));
    assert!(!row.detail.contains('…'));
}

#[test]
fn every_projection_reports_what_it_dropped() {
    let (section, _certificate, source) = bound();
    let bundle = project_all(&section, &events(), source).expect("projects four ways");

    for (kind, dropped) in bundle.fidelity_summary() {
        assert!(dropped > 0, "{kind} claimed a lossless projection");
    }
    for report in [
        bundle.graph.fidelity(),
        bundle.hypergraph.fidelity(),
        bundle.timeline.fidelity(),
        bundle.table.fidelity(),
    ] {
        assert!(!report.is_lossless());
        assert!(report
            .dropped
            .iter()
            .all(|dropped| !dropped.recover_from.is_empty()));
        assert_eq!(report.carried_obligations.len(), 2);
        assert_eq!(report.carried_conflicts.len(), 1);
    }
}

#[test]
fn all_four_projections_of_one_region_share_one_provenance() {
    let (section, _certificate, source) = bound();
    let bundle = project_all(&section, &events(), source).expect("projects four ways");

    let digest = bundle.graph.source().section_sha256().to_string();
    assert_eq!(bundle.hypergraph.source().section_sha256(), digest);
    assert_eq!(bundle.timeline.source().section_sha256(), digest);
    assert_eq!(bundle.table.source().section_sha256(), digest);

    assert_eq!(bundle.graph.kind(), ProjectionKind::Graph);
    assert_eq!(bundle.hypergraph.kind(), ProjectionKind::Hypergraph);
    assert_eq!(bundle.timeline.kind(), ProjectionKind::Timeline);
    assert_eq!(bundle.table.kind(), ProjectionKind::Table);
}

#[test]
fn a_serialised_view_carries_its_provenance_and_its_loss_ledger() {
    let (section, _certificate, source) = bound();
    let view = GraphProjection::new()
        .project(&section, source)
        .expect("projects");

    let wire = serde_json::to_value(&view).expect("serialises");
    assert_eq!(wire["kind"], "graph");
    assert_eq!(wire["source"]["query_id"], "query.split_integrity");
    assert_eq!(
        wire["source"]["section_sha256"],
        section.content_hash().unwrap().as_str()
    );
    assert!(wire["source"]["certificate_sha256"].is_string());
    assert!(wire["fidelity"]["dropped"].as_array().unwrap().len() >= 2);
    assert_eq!(
        wire["fidelity"]["carried_obligations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(wire["body"]["nodes"].is_array());
}

#[test]
fn a_malformed_factor_document_is_a_typed_error_rather_than_a_guessed_shape() {
    let mut section = section();
    section.selected_factors = vec![json!({ "id": "factor.nameless" })];
    let certificate = certificate_for(&section);
    let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
        .expect("binds");

    let error = GraphProjection::new()
        .project(&section, source)
        .expect_err("no signature, no projection");
    match error {
        ProjectionError::MalformedFactor { index, detail } => {
            assert_eq!(index, 0);
            assert!(detail.contains("kind"));
        }
        other => panic!("expected a malformed factor, got {other}"),
    }
}

#[test]
fn the_timeline_projects_the_event_structure_of_a_generated_world() {
    // Hand-built events are convenient but prove nothing about real `fiber-world/0.1` documents.
    // This runs the same projection over a world produced by the structural generator of 43.39.
    let generated = bioprism_worldgen::generate(&WorldSpec::discriminating(64));
    let world = World::from_json(generated.world).expect("generated world parses");
    let (section, _certificate, source) = bound();

    let view = TimelineProjection::new(&world.events)
        .project(&section, source)
        .expect("projects");
    let body = view.body();

    assert_eq!(body.entries.len(), world.events.len());
    assert!(body.dangling_parents.is_empty());
    assert!(body.causal_cycle_members.is_empty());
    assert_eq!(
        body.withheld_at_cut()
            .map(|entry| entry.event_id.as_str())
            .collect::<Vec<_>>(),
        vec!["event.future_label"],
        "the label was generated before the cut but released after it"
    );
    assert_eq!(
        body.entry("event.future_label")
            .unwrap()
            .order_justification,
        OrderJustification::CausalPrecedence
    );
}

#[test]
fn a_region_with_no_obstructions_still_reports_its_flattening() {
    let mut section = section();
    section.unresolved_obligations.clear();
    section.oracle = OracleVerdict::new("deterministic_split_integrity_v1", vec![]);
    let certificate = certificate_for(&section);
    let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Extended)
        .expect("binds");

    let bundle = project_all(&section, &events(), source).expect("projects four ways");
    assert!(bundle.obstructions_survive_everywhere(&section));
    assert_eq!(bundle.table.body().obstruction_rows, 0);
    assert!(bundle.graph.fidelity().has_semantic_loss());
    assert_eq!(bundle.graph.fidelity().carried_obligations.len(), 0);
}

#[test]
fn a_bundle_refuses_to_render_when_the_section_changed_after_provenance_was_bound() {
    let (mut section, _certificate, source) = bound();
    section.goal = "quietly repurposed after the certificate was issued".into();

    let error = project_all(&section, &events(), source).expect_err("the section drifted");
    assert!(matches!(
        error,
        ProjectionError::SectionMutatedAfterBinding { .. }
    ));
}

#[test]
fn re_establishing_a_lapsed_binding_refuses_a_section_that_changed_in_the_meantime() {
    let (mut section, _certificate, source) = bound();
    section.decision_time = "2099-01-01T00:00:00Z".into();

    let error = BoundSection::rebind(&section, source).expect_err("the section drifted");
    match error {
        ProjectionError::SectionMutatedAfterBinding { bound, actual } => {
            assert_ne!(bound, actual);
            assert_eq!(
                actual,
                section.content_hash().expect("section digests").as_str()
            );
        }
        other => panic!("expected a mutation refusal, got {other}"),
    }
}

#[test]
fn a_live_binding_projects_exactly_what_a_detached_source_projects() {
    // The optimisation is only sound if skipping the per-projection guard changes nothing about
    // what is rendered or what provenance is sealed into it. Compare the whole bundle, including
    // every view's `ProjectionSource`, rather than a summary of it.
    let section = section();
    let certificate = certificate_for(&section);

    let detached = ProjectionSource::bind(&section, &certificate, CertificateProfile::Extended)
        .expect("certificate attests this section");
    let by_source = project_all(&section, &events(), detached).expect("projects four ways");

    let bound = BoundSection::bind(&section, &certificate, CertificateProfile::Extended)
        .expect("certificate attests this section");
    assert_eq!(bound.section().query_id, section.query_id);
    assert_eq!(
        bound.source().section_sha256(),
        by_source.graph.source().section_sha256()
    );
    let by_binding = bound.project_all(&events()).expect("projects four ways");

    assert_eq!(by_binding, by_source);
    assert_eq!(
        serde_json::to_string(&by_binding).unwrap(),
        serde_json::to_string(&by_source).unwrap()
    );
}

#[test]
fn a_live_binding_refuses_a_certificate_that_attests_a_different_section() {
    // `BoundSection` is a cheaper way to hold a binding, never a laxer way to make one: it
    // delegates to the same constructor, so the forgery guard is the same guard.
    let section = section();
    let mut other = section.clone();
    other.goal = "a different decision entirely".into();
    let certificate = certificate_for(&other);

    let error = BoundSection::bind(&section, &certificate, CertificateProfile::Reference)
        .expect_err("the certificate attests another section");
    assert!(matches!(
        error,
        ProjectionError::CertificateAttestsAnotherSection { .. }
    ));
}

#[test]
fn a_binding_released_back_into_a_detached_source_is_guarded_again() {
    // `into_source` hands back the weaker object on purpose. Once the borrow is gone the section
    // can move again, so the runtime guard has to take over from the type system — and does.
    let mut section = section();
    let certificate = certificate_for(&section);
    let released = BoundSection::bind(&section, &certificate, CertificateProfile::Extended)
        .expect("certificate attests this section")
        .into_source();

    section.goal = "changed once the borrow was gone".into();

    let error = GraphProjection::new()
        .project(&section, released)
        .expect_err("the section drifted after the binding was released");
    assert!(matches!(
        error,
        ProjectionError::SectionMutatedAfterBinding { .. }
    ));
}
