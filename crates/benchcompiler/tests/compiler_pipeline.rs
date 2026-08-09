//! Invariants of the assembled compiler (06.01) and the stages that feed it: 06.02/06.03
//! segmentation and boundary typing, 06.04 candidate actions, 06.06 attribution, 06.09
//! counterfactual pairs.

use bioprism_benchcompiler::actions::{CandidateAction, CandidateActionSet, Provenance};
use bioprism_benchcompiler::attribute::{
    assert_with, failure_card, Blame, Citation, ConstraintOutcome, ConstraintRecord,
    ConstraintSource,
};
use bioprism_benchcompiler::boundary::{boundaries, episodes, repetitions, DecisionType, Repetition};
use bioprism_benchcompiler::causal::analyse;
use bioprism_benchcompiler::counterfactual::{
    contrast, pair, ContrastOutcome, ExpectedResponse, Intervention, InterventionTarget,
    NoRealismReview,
};
use bioprism_benchcompiler::minimize::{ContextItem, InterestSignature, MinimizeBudget, Tier};
use bioprism_benchcompiler::pipeline::{compile, OutputClass};
use bioprism_benchcompiler::{ActionError, CompileError, CounterfactualError, OracleError};
use bioprism_prism::{DecisionCell, InputRef};
use bioprism_trace::{Event, EventKind, Trace};
use serde_json::json;
use std::collections::BTreeSet;

fn chain(tool_at_three: &str) -> Vec<Event> {
    vec![
        Event::new(0, EventKind::Goal, json!({"summary": "rank the candidates"})),
        Event::new(
            1,
            EventKind::Action,
            json!({"tool": "choose_assay", "irreversible": true}),
        )
        .caused_by(0),
        Event::new(2, EventKind::Result, json!({"summary": "assay selected"})).caused_by(1),
        Event::new(3, EventKind::Action, json!({"tool": tool_at_three})).caused_by(2),
        Event::new(4, EventKind::Claim, json!({"summary": "reported a hit"})).caused_by(3),
        Event::new(5, EventKind::Termination, json!({"summary": "done"})).caused_by(4),
    ]
}

fn context() -> Vec<ContextItem> {
    vec![
        ContextItem::new("panel_manifest", Tier::Artifact),
        ContextItem::new("unused_service", Tier::Service),
        ContextItem::new("stale_memory", Tier::MemoryEntry),
    ]
}

fn probe(kept: &BTreeSet<String>) -> InterestSignature {
    if kept.contains("panel_manifest") {
        InterestSignature::new("invalid").with_witness("identity_leakage")
    } else {
        InterestSignature::new("valid")
    }
}

fn compiled() -> bioprism_benchcompiler::Compilation {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let mut probe_fn = probe;
    compile(
        &failing,
        Some(&passing),
        &context(),
        &mut probe_fn,
        MinimizeBudget::default(),
        &[],
        Vec::new(),
    )
    .expect("the trajectory compiles")
}

#[test]
fn a_compilation_stops_at_a_candidate_cell_and_never_publishes_one_itself() {
    let compilation = compiled();
    assert_eq!(compilation.class, OutputClass::CandidateResearchCell);
    assert_eq!(compilation.cell_step(), Some(3));
    assert!(compilation.oracle.is_some());
}

#[test]
fn a_compilation_cannot_produce_a_cell_without_a_named_reviewer() {
    let compilation = compiled();
    let world = InputRef::new("world.json", &json!({"facts": []}));
    let query = InputRef::new("query.json", &json!({"variable": "panel"}));
    match compilation.approve("  ", world, query) {
        Err(CompileError::Oracle(OracleError::UnattributedReview)) => {}
        other => panic!("expected the review gate to refuse, got {other:?}"),
    }
}

#[test]
fn approval_produces_a_prism_cell_carrying_the_reviewed_contract() {
    let compilation = compiled();
    let world = InputRef::new("world.json", &json!({"facts": []}));
    let query = InputRef::new("query.json", &json!({"variable": "panel"}));
    let (cell, reviewed) = compilation
        .approve("k.okafor", world, query)
        .expect("a named reviewer clears the gate");

    assert_eq!(reviewed.reviewer(), "k.okafor");
    assert!(cell.acceptable_verdicts.contains("invalid"));
    assert!(cell.required_witnesses.contains("identity_leakage"));
    assert_eq!(cell.cell_id, "dc_run_fail#step3");
}

#[test]
fn a_compilation_of_an_environment_divergence_is_rejected_and_says_why() {
    let base = |summary: &str| {
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "rank"})),
            Event::new(1, EventKind::Action, json!({"tool": "search"})).caused_by(0),
            Event::new(2, EventKind::Observation, json!({"summary": summary})).caused_by(1),
            Event::new(3, EventKind::Termination, json!({"summary": "done"})).caused_by(2),
        ]
    };
    let failing = Trace::new("run_fail", base("4 hits"), false);
    let passing = Trace::new("run_pass", base("91 hits"), true);
    let mut probe_fn = probe;
    let compilation = compile(
        &failing,
        Some(&passing),
        &context(),
        &mut probe_fn,
        MinimizeBudget::default(),
        &[],
        Vec::new(),
    )
    .expect("compiles to a rejection, which is still a result");

    match &compilation.class {
        OutputClass::RejectedOrUnresolved { reason } => {
            assert!(reason.contains("observation"));
            assert!(reason.contains("did not control"));
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
    assert!(compilation.oracle.is_none());
}

#[test]
fn compiler_confidence_names_its_unmeasured_stages_instead_of_averaging_them_away() {
    let compilation = compiled();
    let unmeasured = compilation.confidence.unmeasured_stages();
    assert!(unmeasured.contains(&"state_reconstruction"));
    assert!(unmeasured.contains(&"oracle_adequacy"));
    assert!(unmeasured.contains(&"mutation_validity"));

    let (stage, _) = compilation
        .confidence
        .limiting_stage()
        .expect("at least one stage was measured");
    assert!(stage == "boundary_detection" || stage == "minimization_fidelity");
}

#[test]
fn every_compiled_output_field_names_the_rule_that_produced_it() {
    let compilation = compiled();
    let fields: Vec<&str> = compilation
        .provenance
        .iter()
        .map(|entry| entry.output_field.as_str())
        .collect();
    assert!(fields.contains(&"analysis.verdict"));
    assert!(fields.contains(&"card.blame"));
    assert!(fields.contains(&"minimization.minimal"));
    assert!(compilation
        .provenance
        .iter()
        .all(|entry| !entry.rule.is_empty()));
}

#[test]
fn an_uncited_claim_is_a_hypothesis_and_does_not_reach_the_findings() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    let card = failure_card(
        &analysis,
        &[],
        None,
        vec![
            assert_with("the panel was wrong", vec![Citation::Event { step: 3 }]),
            assert_with("the agent was probably rushing", vec![]),
        ],
    );

    assert_eq!(card.findings.len(), 1);
    assert_eq!(card.hypotheses.len(), 1);
    assert_eq!(card.hypotheses[0].claim(), "the agent was probably rushing");
    assert!((card.evidence_ratio() - 0.5).abs() < 1e-9);
}

#[test]
fn a_contradiction_inside_the_task_is_a_benchmark_defect_not_an_agent_failure() {
    let failing = Trace::new("run_fail", chain("run_wrong_panel"), false);
    let passing = Trace::new("run_pass", chain("run_right_panel"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");
    assert_eq!(analysis.first_causal_step(), Some(3));

    let ledger = vec![ConstraintRecord {
        id: "instruction_use_panel_a".to_string(),
        description: "the task says to use panel A".to_string(),
        source: ConstraintSource::TaskInstruction,
        outcome: ConstraintOutcome::Unsatisfiable {
            conflicts_with: "tool_contract_panel_a_unavailable".to_string(),
        },
    }];

    let card = failure_card(&analysis, &ledger, None, Vec::new());
    assert!(matches!(card.blame, Blame::TaskDefect { .. }));
    assert!(!card.blame.counts_against_the_agent());
    assert!(
        card.recommended_cell_steps.is_empty(),
        "a cell extracted from a contradictory task would test the contradiction"
    );
}

#[test]
fn an_environment_divergence_never_produces_agent_blame() {
    let base = |summary: &str| {
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "rank"})),
            Event::new(1, EventKind::Action, json!({"tool": "search"})).caused_by(0),
            Event::new(2, EventKind::Observation, json!({"summary": summary})).caused_by(1),
            Event::new(3, EventKind::Termination, json!({"summary": "done"})).caused_by(2),
        ]
    };
    let failing = Trace::new("run_fail", base("4 hits"), false);
    let passing = Trace::new("run_pass", base("91 hits"), true);
    let analysis = analyse(&failing, Some(&passing)).expect("analysable");

    let card = failure_card(&analysis, &[], None, Vec::new());
    assert_eq!(card.blame, Blame::Environment { at_step: 2 });
    assert!(!card.blame.counts_against_the_agent());
    assert!(!card.alternative_explanations.is_empty());
}

#[test]
fn a_repeated_action_with_no_new_evidence_is_stuck_not_iterative_refinement() {
    let stuck = Trace::new(
        "run_stuck",
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "find it"})),
            Event::new(1, EventKind::Action, json!({"tool": "retry_fetch"})),
            Event::new(2, EventKind::Action, json!({"tool": "retry_fetch"})),
            Event::new(3, EventKind::Action, json!({"tool": "retry_fetch"})),
        ],
        false,
    );
    let refining = Trace::new(
        "run_refine",
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "find it"})),
            Event::new(1, EventKind::Action, json!({"tool": "retry_fetch"})),
            Event::new(2, EventKind::Observation, json!({"summary": "index warmed"}))
                .seeing(["warm_index"]),
            Event::new(3, EventKind::Action, json!({"tool": "retry_fetch"})),
        ],
        true,
    );

    assert_eq!(
        repetitions(&stuck)[0].classification,
        Repetition::Stuck { repeats: 3 }
    );
    assert!(matches!(
        repetitions(&refining)[0].classification,
        Repetition::IterativeRefinement { .. }
    ));
}

#[test]
fn a_trace_is_partitioned_into_one_episode_per_stated_goal() {
    let trace = Trace::new(
        "run",
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "load the cohort"})),
            Event::new(1, EventKind::Action, json!({"tool": "load"})),
            Event::new(2, EventKind::Goal, json!({"summary": "rank the panel"})),
            Event::new(3, EventKind::Action, json!({"tool": "rank"})),
        ],
        true,
    );
    let episodes = episodes(&trace);
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].goal_step, Some(0));
    assert_eq!(episodes[1].label, "rank the panel");
    assert_eq!(episodes[1].steps, vec![2, 3]);
}

#[test]
fn a_decision_no_rule_recognises_is_labelled_unclassified_rather_than_defaulted() {
    let trace = Trace::new(
        "run",
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "go"})),
            Event::new(1, EventKind::Action, json!({"tool": "a", "alternatives": [{"tool": "b"}]})),
            Event::new(2, EventKind::Action, json!({"tool": "a", "alternatives": [{"tool": "a"}]})),
            Event::new(3, EventKind::Action, json!({"tool": "opaque"})).seeing(["fresh"]),
        ],
        false,
    );
    let boundaries = boundaries(&trace, None);
    let at = |step: usize| {
        boundaries
            .iter()
            .find(|boundary| boundary.step == step)
            .unwrap_or_else(|| panic!("step {step} is a boundary"))
    };

    assert_eq!(at(1).decision_type, DecisionType::ToolSelection);
    assert_eq!(at(2).decision_type, DecisionType::ToolArguments);
    assert_eq!(at(3).decision_type, DecisionType::Unclassified);
    assert!(at(3).type_evidence.contains("not defaulted"));
}

#[test]
fn a_forced_step_with_no_alternatives_and_no_new_evidence_is_not_a_standalone_cell() {
    let trace = Trace::new(
        "run",
        vec![
            Event::new(0, EventKind::Goal, json!({"summary": "go"})),
            Event::new(1, EventKind::Action, json!({"tool": "serialise"})).caused_by(0),
            Event::new(2, EventKind::Action, json!({"tool": "pick", "alternatives": [{"tool": "other"}]})),
        ],
        false,
    );
    let boundaries = boundaries(&trace, None);
    let forced = boundaries
        .iter()
        .find(|boundary| boundary.step == 1)
        .expect("step 1 is decision-bearing");
    assert!(!forced.extractable());
    assert!(forced
        .no_op_reason
        .as_deref()
        .unwrap_or_default()
        .contains("forced by step 0"));

    let real = boundaries
        .iter()
        .find(|boundary| boundary.step == 2)
        .expect("step 2 is decision-bearing");
    assert!(real.extractable());
}

#[test]
fn an_action_set_cannot_be_built_on_a_step_the_agent_did_not_control() {
    let trace = Trace::new("run", chain("run_wrong_panel"), false);
    assert_eq!(
        CandidateActionSet::reconstruct(&trace, 2),
        Err(ActionError::NotDecisionBearing {
            step: 2,
            kind: "result"
        })
    );
    assert_eq!(
        CandidateActionSet::reconstruct(&trace, 99),
        Err(ActionError::StepNotInTrace { step: 99 })
    );
}

#[test]
fn a_candidate_claiming_it_was_visible_at_a_later_step_is_a_hindsight_leak() {
    let trace = Trace::new("run", chain("run_wrong_panel"), false);
    let mut set = CandidateActionSet::reconstruct(&trace, 3).expect("step 3 is an action");

    let error = set
        .add(CandidateAction::new(
            "consult the panel manifest",
            Provenance::VisibleAtDecisionTime { from_step: 4 },
        ))
        .expect_err("step 4 had not happened yet");
    assert_eq!(
        error,
        ActionError::HindsightLeak {
            action: "consult the panel manifest".to_string(),
            from_step: 4,
            decision_step: 3,
        }
    );
}

#[test]
fn a_future_sourced_option_may_validate_the_set_but_is_never_shown_to_the_agent() {
    let trace = Trace::new("run", chain("run_wrong_panel"), false);
    let mut set = CandidateActionSet::reconstruct(&trace, 3).expect("step 3 is an action");

    set.add(
        CandidateAction::new(
            "use the panel the grader expected",
            Provenance::FromFuture { from_step: 5 },
        ),
    )
    .expect("future-sourced options are permitted for validation");
    set.add(
        CandidateAction::new(
            "read the manifest first",
            Provenance::VisibleAtDecisionTime { from_step: 2 },
        )
        .accomplishing("inspect the panel contract before committing")
        .strong(),
    )
    .expect("visible at decision time");
    set.add(
        CandidateAction::new("call the retired endpoint", Provenance::ToolSchema {
            tool: "legacy_panel".to_string(),
        })
        .infeasible("the tool was removed before this run"),
    )
    .expect("infeasible options are diagnostic hypotheses");

    assert!(set
        .visible_to_agent()
        .iter()
        .all(|action| action.label != "use the panel the grader expected"));
    assert_eq!(set.validation_only().len(), 1);
    assert!(set
        .acceptable()
        .iter()
        .all(|action| action.label != "call the retired endpoint"));

    let coverage = set.coverage();
    assert_eq!(coverage.validation_only, 1);
    assert!(coverage.adequate);
}

fn cell(id: &str, world: serde_json::Value) -> DecisionCell {
    DecisionCell::new(
        id,
        "step 3: chose the panel",
        InputRef::new("world.json", &world),
        InputRef::new("query.json", &json!({"variable": "panel"})),
    )
}

#[test]
fn a_counterfactual_pair_that_moved_two_things_is_refused() {
    let source = cell("dc_source", json!({"permission": "read"}));
    let mut followup = cell("dc_followup", json!({"permission": "write"}));
    followup.query = InputRef::new("query.json", &json!({"variable": "assay"}));

    let intervention = Intervention::new(
        "write permission",
        InterventionTarget::Permission,
        json!("read"),
        json!("write"),
    )
    .changing("world");

    let error = pair(
        source,
        followup,
        intervention,
        ExpectedResponse::Invariant {
            rationale: "the panel choice does not depend on write access".to_string(),
        },
        &mut NoRealismReview,
        false,
    )
    .expect_err("a pair that moved the query too measures neither change");
    assert_eq!(
        error,
        CounterfactualError::UnmatchedPair {
            fields: vec!["query".to_string()]
        }
    );
}

#[test]
fn a_null_intervention_is_refused_rather_than_recorded_as_a_contrast() {
    let source = cell("dc_source", json!({"permission": "read"}));
    let followup = cell("dc_followup", json!({"permission": "read"}));
    let intervention = Intervention::new(
        "write permission",
        InterventionTarget::Permission,
        json!("read"),
        json!("read"),
    )
    .changing("world");

    assert_eq!(
        pair(
            source,
            followup,
            intervention,
            ExpectedResponse::Invariant {
                rationale: "irrelevant".to_string()
            },
            &mut NoRealismReview,
            false,
        )
        .unwrap_err(),
        CounterfactualError::NullIntervention {
            factor: "write permission".to_string()
        }
    );
}

#[test]
fn an_intervention_the_callers_realism_check_rejects_never_becomes_a_pair() {
    let source = cell("dc_source", json!({"permission": "read"}));
    let followup = cell("dc_followup", json!({"permission": "write"}));
    let intervention = Intervention::new(
        "write permission",
        InterventionTarget::Permission,
        json!("read"),
        json!("write"),
    )
    .changing("world");

    let mut refuses = |_: &Intervention| Err("the sandbox is read-only by construction".to_string());
    assert!(matches!(
        pair(
            source,
            followup,
            intervention,
            ExpectedResponse::Invariant {
                rationale: "irrelevant".to_string()
            },
            &mut refuses,
            true,
        )
        .unwrap_err(),
        CounterfactualError::IncoherentState { .. }
    ));
}

#[test]
fn a_candidate_that_moves_on_an_invariant_pair_is_reported_as_spurious_sensitivity() {
    let source = cell("dc_source", json!({"wording": "which panel"}));
    let followup = cell("dc_followup", json!({"wording": "select the panel"}));
    let intervention = Intervention::new(
        "instruction wording",
        InterventionTarget::UserIntent,
        json!("which panel"),
        json!("select the panel"),
    )
    .changing("world");

    let built = pair(
        source,
        followup,
        intervention,
        ExpectedResponse::Invariant {
            rationale: "wording carries no constraint".to_string(),
        },
        &mut NoRealismReview,
        true,
    )
    .expect("matched");

    assert_eq!(contrast(&built, "valid", "valid"), ContrastOutcome::AsPredicted);
    assert_eq!(
        contrast(&built, "valid", "invalid"),
        ContrastOutcome::SpuriousSensitivity {
            moved_to: "invalid".to_string()
        }
    );
}

#[test]
fn a_candidate_that_ignores_a_must_change_pair_is_reported_as_missing_the_change() {
    let source = cell("dc_source", json!({"permission": "read"}));
    let followup = cell("dc_followup", json!({"permission": "none"}));
    let intervention = Intervention::new(
        "read permission",
        InterventionTarget::Permission,
        json!("read"),
        json!("none"),
    )
    .changing("world");

    let built = pair(
        source,
        followup,
        intervention,
        ExpectedResponse::MustChange {
            to_verdicts: ["underdetermined".to_string()].into_iter().collect(),
            rationale: "with no read permission the evidence cannot be obtained".to_string(),
        },
        &mut NoRealismReview,
        true,
    )
    .expect("matched");

    assert_eq!(
        contrast(&built, "valid", "underdetermined"),
        ContrastOutcome::AsPredicted
    );
    assert_eq!(
        contrast(&built, "valid", "valid"),
        ContrastOutcome::MissedTheChange {
            stayed_at: "valid".to_string()
        }
    );
    assert_eq!(
        contrast(&built, "valid", "invalid"),
        ContrastOutcome::WrongDirection {
            moved_to: "invalid".to_string()
        }
    );
}

#[test]
fn a_pair_with_colliding_cell_ids_is_refused() {
    let source = cell("dc_same", json!({"permission": "read"}));
    let followup = cell("dc_same", json!({"permission": "write"}));
    let intervention = Intervention::new(
        "permission",
        InterventionTarget::Permission,
        json!("read"),
        json!("write"),
    )
    .changing("world");

    assert_eq!(
        pair(
            source,
            followup,
            intervention,
            ExpectedResponse::Invariant {
                rationale: "irrelevant".to_string()
            },
            &mut NoRealismReview,
            true,
        )
        .unwrap_err(),
        CounterfactualError::CollidingCellIds {
            cell_id: "dc_same".to_string()
        }
    );
}
