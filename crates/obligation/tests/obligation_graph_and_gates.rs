use bioprism_obligation::{
    may_perform, Action, BlockReason, Gate, Obligation, ObligationError, ObligationGraph,
    ObligationPredicate, ObligationState, RegretClass, StateRecord,
};
use bioprism_scope::Timestamp;

fn at() -> Timestamp {
    Timestamp::parse("2026-08-07T00:00:00Z").expect("fixture timestamp parses")
}

fn satisfied(actor: &str, confidence: f64) -> StateRecord {
    StateRecord::new(ObligationState::Satisfied, actor, at(), confidence)
        .with_evidence(["fact.identity_map"])
}

/// The worked example of blueprint 39.06: a radiogenomic validation audit whose modelling step
/// sits downstream of identity, temporal firewall, split independence, measurement provenance and
/// estimand obligations.
fn audit_graph() -> ObligationGraph {
    let mut graph = ObligationGraph::new("audit radiogenomic validation");
    for (id, statement) in [
        ("identity", "subject and specimen identity resolved"),
        ("temporal_firewall", "no evidence released after the cut"),
        ("split_unit", "split unit is independent of the subject"),
        (
            "measurement_provenance",
            "assay and preprocessing versions known",
        ),
        ("estimand", "estimand matches the claim"),
    ] {
        graph
            .insert(Obligation::new(id, statement).mandatory())
            .expect("fixture obligation is unique");
    }
    graph
        .insert(
            Obligation::new("modeling_allowed", "modelling may proceed").depending_on([
                "identity",
                "temporal_firewall",
                "split_unit",
                "measurement_provenance",
                "estimand",
            ]),
        )
        .expect("fixture obligation is unique");
    graph
}

fn discharge_all(graph: &mut ObligationGraph) {
    for id in [
        "identity",
        "temporal_firewall",
        "split_unit",
        "measurement_provenance",
        "estimand",
        "modeling_allowed",
    ] {
        graph
            .record(id, satisfied("auditor", 0.9))
            .expect("fixture obligation exists");
    }
}

#[test]
fn an_unseen_obligation_carries_no_record_and_no_confidence() {
    let graph = audit_graph();
    let obligation = graph.get("identity").expect("obligation is present");
    assert_eq!(obligation.recorded_state(), ObligationState::Unseen);
    assert!(obligation.current().is_none());
    assert_eq!(obligation.confidence(), 0.0);
}

#[test]
fn a_satisfied_state_without_an_evidence_locator_is_rejected() {
    let mut graph = audit_graph();
    let error = graph
        .record(
            "identity",
            StateRecord::new(ObligationState::Satisfied, "auditor", at(), 1.0),
        )
        .expect_err("satisfaction without evidence must not be recordable");
    assert!(matches!(error, ObligationError::EvidenceRequired { .. }));
}

#[test]
fn a_waiver_without_a_written_reason_is_rejected() {
    let mut graph = audit_graph();
    let error = graph
        .record(
            "split_unit",
            StateRecord::new(ObligationState::WaivedWithReason, "reviewer", at(), 0.5),
        )
        .expect_err("a waiver is only admissible with its reason");
    assert!(matches!(error, ObligationError::ReasonRequired { .. }));
}

#[test]
fn a_state_record_without_an_actor_is_rejected() {
    let mut graph = audit_graph();
    let error = graph
        .record("identity", satisfied("   ", 1.0))
        .expect_err("an unattributed assertion cannot be audited");
    assert_eq!(
        error,
        ObligationError::ActorRequired("identity".to_string())
    );
}

#[test]
fn confidence_outside_the_unit_interval_is_rejected() {
    let mut graph = audit_graph();
    let error = graph
        .record("identity", satisfied("auditor", 1.4))
        .expect_err("confidence is bounded");
    assert!(matches!(
        error,
        ObligationError::ConfidenceOutOfRange { .. }
    ));
}

#[test]
fn a_satisfied_obligation_resting_on_an_open_dependency_is_only_partially_supported() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);
    graph
        .record(
            "temporal_firewall",
            StateRecord::new(ObligationState::Open, "auditor", at(), 0.2),
        )
        .expect("obligation exists");

    let effective = graph.effective_states().expect("graph is acyclic");
    assert_eq!(
        graph
            .get("modeling_allowed")
            .expect("obligation is present")
            .recorded_state(),
        ObligationState::Satisfied
    );
    assert_eq!(
        effective["modeling_allowed"],
        ObligationState::PartiallySupported
    );
}

#[test]
fn a_contradicted_dependency_contradicts_everything_downstream() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);
    graph
        .record(
            "split_unit",
            StateRecord::new(ObligationState::Contradicted, "statistician", at(), 0.95)
                .with_evidence(["fact.duplicate_subjects_across_folds"]),
        )
        .expect("obligation exists");

    assert_eq!(
        graph
            .effective_state("modeling_allowed")
            .expect("graph is acyclic"),
        ObligationState::Contradicted
    );
}

#[test]
fn an_inapplicable_obligation_is_not_weakened_by_its_dependencies() {
    let mut graph = audit_graph();
    graph
        .record(
            "modeling_allowed",
            StateRecord::new(ObligationState::NotApplicable, "reviewer", at(), 1.0)
                .with_reason("this audit does not fit a model"),
        )
        .expect("obligation exists");

    assert_eq!(
        graph
            .effective_state("modeling_allowed")
            .expect("graph is acyclic"),
        ObligationState::NotApplicable
    );
}

#[test]
fn a_dependency_cycle_is_reported_rather_than_looped() {
    let mut graph = ObligationGraph::new("circular goal");
    graph
        .insert(Obligation::new("a", "a").depending_on(["b"]))
        .expect("unique");
    graph
        .insert(Obligation::new("b", "b").depending_on(["a"]))
        .expect("unique");

    let error = graph.validate().expect_err("a cycle is not a graph");
    assert!(matches!(error, ObligationError::DependencyCycle(_)));
}

#[test]
fn a_dependency_on_an_obligation_outside_the_graph_is_rejected() {
    let mut graph = ObligationGraph::new("incomplete goal");
    graph
        .insert(Obligation::new("modeling_allowed", "model").depending_on(["identity"]))
        .expect("unique");

    let error = graph.validate().expect_err("the dependency is missing");
    assert_eq!(
        error,
        ObligationError::DanglingDependency {
            obligation: "modeling_allowed".into(),
            missing: "identity".into(),
        }
    );
}

#[test]
fn a_high_regret_action_is_blocked_while_its_prerequisite_obligation_is_open() {
    let mut graph = audit_graph();
    graph
        .record(
            "identity",
            StateRecord::new(ObligationState::Open, "auditor", at(), 0.1),
        )
        .expect("obligation exists");

    let action = Action::new("publish_validation_report", RegretClass::HighRegret)
        .described("publish an external-generalisation claim")
        .requiring(ObligationPredicate::satisfied("identity"));

    let gate = may_perform(&action, &graph);
    assert!(gate.is_blocked());
    assert_eq!(gate.block_reason(), Some(&BlockReason::PrerequisitesUnmet));
    assert_eq!(gate.unmet()[0].obligation, "identity");
    assert_eq!(gate.unmet()[0].effective, ObligationState::Open);
}

#[test]
fn a_high_regret_action_declaring_no_prerequisites_is_blocked_rather_than_allowed() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);

    let action = Action::new("publish_validation_report", RegretClass::HighRegret);
    let gate = may_perform(&action, &graph);

    assert!(
        gate.is_blocked(),
        "an ungated high-regret action must not slip through on an empty prerequisite list"
    );
    assert_eq!(
        gate.block_reason(),
        Some(&BlockReason::UndeclaredPrerequisites)
    );
}

#[test]
fn a_prerequisite_naming_an_obligation_outside_the_graph_blocks_the_action() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);

    let action = Action::new("release_cohort", RegretClass::HighRegret)
        .requiring(ObligationPredicate::satisfied("consent_checked"));
    let gate = may_perform(&action, &graph);

    assert_eq!(
        gate.block_reason(),
        Some(&BlockReason::UnknownObligation {
            obligation: "consent_checked".into(),
        }),
        "removing the obligation must not be a way to satisfy the predicate"
    );
}

#[test]
fn an_irreversible_action_is_blocked_by_a_mandatory_obligation_it_never_declared() {
    let mut graph = audit_graph();
    graph
        .record("identity", satisfied("auditor", 1.0))
        .expect("obligation exists");

    let action = Action::new("egress_derived_features", RegretClass::Irreversible)
        .requiring(ObligationPredicate::satisfied("identity"));
    let gate = may_perform(&action, &graph);

    match gate.block_reason() {
        Some(BlockReason::MandatoryObligationOutstanding { obligation, .. }) => {
            assert_ne!(obligation, "identity");
        }
        other => panic!("expected an outstanding mandatory obligation, got {other:?}"),
    }
}

#[test]
fn a_gate_that_does_not_accept_waivers_blocks_a_waived_obligation() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);
    graph
        .record(
            "split_unit",
            StateRecord::new(ObligationState::WaivedWithReason, "reviewer", at(), 0.6)
                .with_reason("single-centre cohort, accepted by the review board"),
        )
        .expect("obligation exists");

    let strict = Action::new("publish", RegretClass::HighRegret)
        .requiring(ObligationPredicate::satisfied("split_unit"));
    let permissive = Action::new("publish", RegretClass::HighRegret)
        .requiring(ObligationPredicate::waivable("split_unit"));

    assert!(may_perform(&strict, &graph).is_blocked());
    assert!(may_perform(&permissive, &graph).is_allowed());
}

#[test]
fn a_confidence_floor_blocks_an_otherwise_satisfied_prerequisite() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);
    graph
        .record("estimand", satisfied("statistician", 0.4))
        .expect("obligation exists");

    let action = Action::new("publish", RegretClass::HighRegret)
        .requiring(ObligationPredicate::satisfied("estimand").with_min_confidence(0.8));
    let gate = may_perform(&action, &graph);

    assert!(gate.is_blocked());
    assert_eq!(gate.unmet()[0].min_confidence, 0.8);
    assert_eq!(gate.unmet()[0].effective, ObligationState::Satisfied);
}

#[test]
fn no_action_is_permitted_on_a_graph_that_cannot_be_verified() {
    let mut graph = ObligationGraph::new("circular goal");
    graph
        .insert(Obligation::new("a", "a").depending_on(["b"]))
        .expect("unique");
    graph
        .insert(Obligation::new("b", "b").depending_on(["a"]))
        .expect("unique");

    let harmless = Action::new("read_inventory", RegretClass::Reversible);
    let gate = may_perform(&harmless, &graph);

    assert!(gate.is_blocked());
    assert!(matches!(
        gate.block_reason(),
        Some(BlockReason::GraphUnverifiable { .. })
    ));
}

#[test]
fn a_reversible_action_needs_no_declared_prerequisites() {
    let graph = audit_graph();
    let action = Action::new("list_obligations", RegretClass::Reversible);
    assert!(may_perform(&action, &graph).is_allowed());
}

#[test]
fn a_downgraded_prerequisite_reports_both_the_record_and_what_the_graph_allows_it_to_mean() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);
    graph
        .record(
            "estimand",
            StateRecord::new(ObligationState::Open, "statistician", at(), 0.3),
        )
        .expect("obligation exists");

    let action = Action::new("publish", RegretClass::HighRegret)
        .requiring(ObligationPredicate::satisfied("modeling_allowed"));
    let gate = may_perform(&action, &graph);
    let unmet = &gate.unmet()[0];

    assert_eq!(unmet.recorded, ObligationState::Satisfied);
    assert_eq!(unmet.effective, ObligationState::PartiallySupported);
    assert!(unmet.detail.contains("dependencies cap it"));
}

#[test]
fn the_frontier_offers_only_obligations_whose_dependencies_are_discharged() {
    let mut graph = audit_graph();
    graph
        .record("identity", satisfied("auditor", 0.9))
        .expect("obligation exists");

    let frontier = graph.frontier().expect("graph is acyclic");
    let ids: Vec<&str> = frontier
        .iter()
        .map(|entry| entry.obligation.as_str())
        .collect();

    assert!(
        !ids.contains(&"identity"),
        "discharged obligations are done"
    );
    assert!(
        !ids.contains(&"modeling_allowed"),
        "an obligation with open dependencies is not actionable yet"
    );
    assert!(ids.contains(&"temporal_firewall"));
}

#[test]
fn a_gate_decision_serialises_as_an_audit_record() {
    let mut graph = audit_graph();
    discharge_all(&mut graph);

    let action = Action::new("publish", RegretClass::HighRegret)
        .requiring(ObligationPredicate::satisfied("identity"));
    let gate = may_perform(&action, &graph);
    let document = serde_json::to_value(&gate).expect("gate is serialisable");

    assert_eq!(document["gate"], "allowed");
    assert_eq!(document["action"], "publish");
    assert_eq!(document["checked"][0], "identity");

    let restored: Gate = serde_json::from_value(document).expect("gate round-trips");
    assert_eq!(restored, gate);
}

#[test]
fn obligation_history_is_append_only() {
    let mut graph = audit_graph();
    graph
        .record(
            "identity",
            StateRecord::new(ObligationState::Open, "auditor", at(), 0.1),
        )
        .expect("obligation exists");
    graph
        .record("identity", satisfied("auditor", 0.9))
        .expect("obligation exists");

    let history = graph
        .get("identity")
        .expect("obligation is present")
        .history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].state, ObligationState::Open);
    assert_eq!(history[1].state, ObligationState::Satisfied);
}

#[test]
fn the_eight_states_of_the_specification_round_trip_through_their_names() {
    for state in ObligationState::ALL {
        assert_eq!(ObligationState::parse(state.as_str()), Some(state));
    }
    assert_eq!(ObligationState::ALL.len(), 8);
    assert_eq!(
        ObligationState::WaivedWithReason.as_str(),
        "waived_with_reason"
    );
}
