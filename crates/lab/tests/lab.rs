//! End-to-end invariants across the lab's modules.
//!
//! The unit tests in each module pin that module's rule. These pin the ones that only exist when
//! the modules are used together — in particular the two ways a self-improvement loop tries to
//! launder a burned holdout, which no single module can see on its own.

use bioprism_lab::context_value::{
    expand, AcquisitionAction, AcquisitionCost, AcquisitionKind, StopReason,
};
use bioprism_lab::error::{EvolutionError, HoldoutError};
use bioprism_lab::evolution::{
    ChangeProposal, ContaminationRecord, EvolutionArchive, EvolutionCard,
};
use bioprism_lab::holdout::{Holdout, HoldoutId, HoldoutLedger, Partition};
use bioprism_lab::hypothesis::{separate, Hypothesis, HypothesisSet, Observations};
use bioprism_lab::pareto::{Direction, Objective, ParetoFront, Profile, Selection};
use bioprism_lab::report::LabReport;
use bioprism_lab::risk::{
    BranchAction, BranchCeiling, BranchLedger, BranchOutcome, BranchPolicy, BranchRule,
    Reversibility, RiskFeatures, Trigger, UndeterminedPolicy,
};
use bioprism_lab::rollback::Deployment;
use bioprism_lab::space::{
    ArchitectureSpace, CandidateArchitecture, ComponentKind, ComponentSpec, ConfigurationId,
    ParameterValue,
};
use bioprism_atlas::UnmeasuredReason;
use bioprism_obligation::{Obligation, ObligationGraph, ObligationState, StateRecord};
use bioprism_scope::Timestamp;

const AT: Timestamp = Timestamp::from_nanos_utc(1_700_000_000_000_000_000);

fn candidate(id: &str, depth: i64) -> CandidateArchitecture {
    CandidateArchitecture::new(id)
        .with_component(
            ComponentSpec::new("select", ComponentKind::ContextSelector)
                .with_parameter("closure_depth", ParameterValue::Integer(depth))
                .feeding(["run"]),
        )
        .with_component(ComponentSpec::new("run", ComponentKind::Executor).feeding(["stop"]))
        .with_component(ComponentSpec::new("stop", ComponentKind::Terminator))
        .costing(10)
}

fn space() -> ArchitectureSpace {
    let mut space = ArchitectureSpace::new();
    space.register(candidate("v1", 3)).unwrap();
    space
        .register(candidate("v2", 5).derived_from("v1"))
        .unwrap();
    space
}

fn ledger() -> HoldoutLedger {
    let mut ledger = HoldoutLedger::new();
    ledger
        .register(Holdout::new(
            "dev-panel",
            Partition::ArchitectureDevelopment,
            1_000,
        ))
        .unwrap();
    ledger
        .register(Holdout::new(
            "private-a",
            Partition::RotatingPrivateCertification,
            8,
        ))
        .unwrap();
    ledger
}

fn proposal(space: &ArchitectureSpace) -> ChangeProposal {
    let before = space.get(&ConfigurationId::new("v1")).unwrap();
    let after = space.get(&ConfigurationId::new("v2")).unwrap();
    ChangeProposal::new("p1", "widen the protected closure before the relevance step")
        .changing(before.diff(after))
        .targeting(["cluster:missing-closure"])
        .with_regression_cells(["cell:closure-depth-5"])
}

fn defeaters() -> Vec<String> {
    vec![
        "the gain survives on a second rotating private set".to_string(),
        "no capability regressed beyond the panel's own variation".to_string(),
    ]
}

#[test]
fn searching_on_the_development_panel_leaves_the_certification_surface_clean() {
    let space = space();
    let mut ledger = ledger();
    let dev = HoldoutId::new("dev-panel");
    let private = HoldoutId::new("private-a");

    ledger
        .record_search(
            &dev,
            &[ConfigurationId::new("v1"), ConfigurationId::new("v2")],
        )
        .unwrap();
    ledger
        .record_selection(&dev, &ConfigurationId::new("v2"), "won the panel")
        .unwrap();

    let before = ledger
        .measure(
            &private,
            &space,
            &ConfigurationId::new("v1"),
            "admissible_rate",
            0.70,
        )
        .unwrap();
    let after = ledger
        .measure(
            &private,
            &space,
            &ConfigurationId::new("v2"),
            "admissible_rate",
            0.83,
        )
        .unwrap();

    let card = EvolutionCard::measured(
        "card-1",
        proposal(&space),
        before,
        after,
        &ConfigurationId::new("v1"),
        defeaters(),
    )
    .unwrap();
    let claim = card.claim_improvement(Direction::HigherIsBetter).unwrap();
    assert!((claim.delta() - 0.13).abs() < 1e-9);

    let mut archive = EvolutionArchive::new();
    archive.push(card);
    let report = LabReport::assemble(&archive, &ledger, Direction::HigherIsBetter);
    assert!(!report.blocks_release());
    assert_eq!(report.improvements.len(), 1);
}

#[test]
fn searching_on_the_certification_surface_makes_the_later_improvement_unreportable() {
    let space = space();
    let mut ledger = ledger();
    let private = HoldoutId::new("private-a");

    ledger
        .record_search(
            &private,
            &[ConfigurationId::new("v1"), ConfigurationId::new("v2")],
        )
        .unwrap();

    let refusal = ledger
        .measure(
            &private,
            &space,
            &ConfigurationId::new("v2"),
            "admissible_rate",
            0.83,
        )
        .unwrap_err();
    assert!(matches!(
        refusal,
        HoldoutError::SelectedUsingThisHoldout { .. }
    ));

    let card = EvolutionCard::contaminated(
        "card-2",
        proposal(&space),
        &ConfigurationId::new("v1"),
        &ConfigurationId::new("v2"),
        ContaminationRecord {
            holdout: private,
            configuration: ConfigurationId::new("v2"),
            refusal,
        },
        &ConfigurationId::new("v1"),
        defeaters(),
    );
    assert!(matches!(
        card.claim_improvement(Direction::HigherIsBetter),
        Err(EvolutionError::ContaminatedSurface { .. })
    ));

    let mut archive = EvolutionArchive::new();
    archive.push(card);
    let report = LabReport::assemble(&archive, &ledger, Direction::HigherIsBetter);
    assert!(report.blocks_release());
    assert!(report.improvements.is_empty());
    assert_eq!(report.contaminated_attempts.len(), 1);
}

#[test]
fn tuning_a_parent_on_the_holdout_does_not_let_the_child_be_measured_on_it() {
    let space = space();
    let mut ledger = ledger();
    let private = HoldoutId::new("private-a");

    ledger
        .record_selection(&private, &ConfigurationId::new("v1"), "picked here")
        .unwrap();

    assert_eq!(
        ledger.measure(
            &private,
            &space,
            &ConfigurationId::new("v2"),
            "admissible_rate",
            0.99
        ),
        Err(HoldoutError::AncestorExposed {
            holdout: "private-a".to_string(),
            configuration: "v2".to_string(),
            ancestor: "v1".to_string(),
        })
    );
}

#[test]
fn a_rollback_restores_the_bundle_and_the_receipt_names_what_it_could_not_restore() {
    let mut deployment = Deployment::new(space(), ledger(), ConfigurationId::new("v1")).unwrap();
    let checkpoint = deployment.checkpoint("pre-v2");
    let private = HoldoutId::new("private-a");

    deployment
        .promote(&ConfigurationId::new("v2"), Some(&private), "beat v1")
        .unwrap();
    let receipt = deployment.rollback(&checkpoint).unwrap();

    assert_eq!(deployment.current(), &ConfigurationId::new("v1"));
    assert!(!receipt.is_complete_restoration());
    assert_eq!(
        receipt.permanently_burned(),
        vec![(private.clone(), ConfigurationId::new("v2"))]
    );

    let space = deployment.space.clone();
    assert!(deployment
        .holdouts
        .measure(
            &private,
            &space,
            &ConfigurationId::new("v2"),
            "admissible_rate",
            0.83
        )
        .is_err());
}

#[test]
fn an_unmeasured_safety_axis_keeps_a_candidate_on_the_front_and_the_report_says_why() {
    let mut front = ParetoFront::new(vec![
        Objective::higher_is_better("admissible_rate"),
        Objective::lower_is_better("cost_units"),
        Objective::higher_is_better("safety"),
    ])
    .unwrap();
    front
        .insert(
            Profile::new(&ConfigurationId::new("v1"))
                .measured("admissible_rate", 0.70)
                .measured("cost_units", 10.0)
                .measured("safety", 0.99),
        )
        .unwrap();
    front
        .insert(
            Profile::new(&ConfigurationId::new("v2"))
                .measured("admissible_rate", 0.83)
                .measured("cost_units", 8.0)
                .unmeasured("safety", UnmeasuredReason::NotAttempted),
        )
        .unwrap();

    assert_eq!(front.len(), 2);
    let selection = front.select();
    let Selection::Ambiguous { ref unresolved, .. } = selection else {
        panic!("an unmeasured axis must not resolve to a unique winner");
    };
    assert_eq!(unresolved.len(), 1);

    let markdown = LabReport::default()
        .with_selection(selection)
        .to_markdown();
    assert!(markdown.contains("was never measured"));
}

#[test]
fn unseparated_hypotheses_feed_the_branch_trigger_and_the_ledger_reports_the_cost() {
    let mut set = HypothesisSet::new();
    set.insert(
        Hypothesis::new("retry", "the retry path double-writes", "template:structural")
            .asserting("idempotency_key_stable"),
    )
    .unwrap();
    set.insert(
        Hypothesis::new("clock", "clock skew reorders writes", "template:structural")
            .denying("idempotency_key_stable"),
    )
    .unwrap();

    let mut graph = ObligationGraph::new("decide whether to replay the batch");
    graph
        .insert(Obligation::new("idempotency_key_stable", "is the key stable across retries?").mandatory())
        .unwrap();
    graph
        .record(
            "idempotency_key_stable",
            StateRecord::new(ObligationState::Open, "analyst", AT, 0.6),
        )
        .unwrap();

    let verdict = separate(&set, &graph, &Observations::new()).unwrap();
    assert!(!verdict.licenses_a_winner());

    let features = RiskFeatures {
        reversibility: Reversibility::Irreversible,
        unseparated_hypotheses: set.live().len(),
        unmet_mandatory_obligations: graph.undischarged().unwrap().len(),
        ..RiskFeatures::benign()
    };
    let policy = BranchPolicy::new(
        BranchCeiling {
            max_branches: 4,
            max_verifier_calls: 2,
        },
        UndeterminedPolicy::Escalate,
        vec![BranchRule::new(
            "irreversible-and-contested",
            Trigger::All {
                of: vec![
                    Trigger::ReversibilityAtLeast {
                        level: Reversibility::Irreversible,
                    },
                    Trigger::UnseparatedHypothesesAtLeast { count: 2 },
                ],
            },
            BranchAction::ForkSuffixes,
        )
        .spending(2, 1)],
    )
    .unwrap();

    let plan = policy.plan(&features);
    assert_eq!(plan.action, BranchAction::ForkSuffixes);

    let mut branch_ledger = BranchLedger::new();
    branch_ledger.record(BranchOutcome::new("replay-batch", plan));
    let report = LabReport::default().with_branching(branch_ledger.report());
    assert!(report.blocks_release());
    assert!(report.to_markdown().contains("Paid and caught nothing"));
}

#[test]
fn expansion_orders_acquisition_toward_the_obligation_that_would_separate_the_hypotheses() {
    let mut graph = ObligationGraph::new("decide whether to replay the batch");
    graph
        .insert(Obligation::new("idempotency_key_stable", "is the key stable?").mandatory())
        .unwrap();
    graph
        .insert(Obligation::new("clock_skew", "is the clock skewed?"))
        .unwrap();

    let actions = vec![
        AcquisitionAction::new("read-retry-log", AcquisitionKind::InspectLog, 40.0)
            .targeting(["idempotency_key_stable"])
            .costing(100, 0),
        AcquisitionAction::new("grep-the-repo", AcquisitionKind::SearchRepository, 1.0)
            .targeting(["clock_skew"])
            .costing(50, 0),
        AcquisitionAction::new("read-user-mailbox", AcquisitionKind::ReadRegion, 1_000.0)
            .targeting(["idempotency_key_stable"])
            .costing(1, 0)
            .crossing("permission:no-mailbox-access"),
    ];

    let plan = expand(
        &actions,
        &graph,
        AcquisitionCost::new(1_000, 10),
        0.0,
        None,
    )
    .unwrap();

    assert_eq!(plan.ordered[0].action, "read-retry-log");
    assert!(plan
        .excluded
        .iter()
        .any(|(id, _)| id == "read-user-mailbox"));
    assert_eq!(plan.stop, StopReason::AllActionsPlanned);
}

#[test]
fn a_settled_decision_stops_expansion_before_it_spends_anything() {
    let mut set = HypothesisSet::new();
    set.insert(Hypothesis::new("retry", "retry double-writes", "t").asserting("key_stable"))
        .unwrap();
    set.insert(Hypothesis::new("clock", "clock skew", "t").denying("key_stable"))
        .unwrap();

    let mut graph = ObligationGraph::new("decide");
    graph
        .insert(Obligation::new("key_stable", "is the key stable?"))
        .unwrap();
    graph
        .record(
            "key_stable",
            StateRecord::new(ObligationState::Satisfied, "analyst", AT, 0.95)
                .with_evidence(["trace://retry-log#42"]),
        )
        .unwrap();

    let verdict = separate(&set, &graph, &Observations::new()).unwrap();
    assert!(verdict.licenses_a_winner());

    let actions = vec![
        AcquisitionAction::new("more-logs", AcquisitionKind::InspectLog, 5.0)
            .targeting(["key_stable"])
            .costing(500, 0),
    ];
    let plan = expand(
        &actions,
        &graph,
        AcquisitionCost::new(1_000, 10),
        0.0,
        Some(&verdict),
    )
    .unwrap();
    assert_eq!(plan.spent, AcquisitionCost::default());
    assert!(matches!(
        plan.stop,
        StopReason::DecisionRobustAcrossHypotheses { .. }
    ));
}

#[test]
fn a_lab_report_round_trips_through_json_even_though_the_claims_behind_it_do_not() {
    let space = space();
    let mut ledger = ledger();
    let private = HoldoutId::new("private-a");
    let before = ledger
        .measure(&private, &space, &ConfigurationId::new("v1"), "rate", 0.70)
        .unwrap();
    let after = ledger
        .measure(&private, &space, &ConfigurationId::new("v2"), "rate", 0.83)
        .unwrap();
    let mut archive = EvolutionArchive::new();
    archive.push(
        EvolutionCard::measured(
            "card-1",
            proposal(&space),
            before,
            after,
            &ConfigurationId::new("v1"),
            defeaters(),
        )
        .unwrap(),
    );

    let report = LabReport::assemble(&archive, &ledger, Direction::HigherIsBetter);
    let json = serde_json::to_string(&report).unwrap();
    let decoded: LabReport = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, report);
}

#[test]
fn spending_the_last_certification_query_retires_the_surface_and_blocks_the_release() {
    let space = space();
    let mut ledger = HoldoutLedger::new();
    ledger
        .register(Holdout::new(
            "private-a",
            Partition::RotatingPrivateCertification,
            2,
        ))
        .unwrap();
    let private = HoldoutId::new("private-a");
    ledger
        .measure(&private, &space, &ConfigurationId::new("v1"), "rate", 0.70)
        .unwrap();
    ledger
        .measure(&private, &space, &ConfigurationId::new("v2"), "rate", 0.83)
        .unwrap();

    assert!(ledger.remaining_certification_budget().is_empty());
    let report = LabReport::assemble(
        &EvolutionArchive::new(),
        &ledger,
        Direction::HigherIsBetter,
    );
    assert_eq!(report.retired_surfaces, vec!["private-a".to_string()]);
    assert!(report.blocks_release());
}
