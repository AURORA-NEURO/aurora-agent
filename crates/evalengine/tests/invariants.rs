//! End-to-end invariants, exercised across modules rather than within one.
//!
//! Each test states a claim the blueprint makes about the evaluation engine and shows that the
//! crate holds it through a full path: contributions in, composition, aggregation, gate. Unit
//! tests in each module cover the mechanics; these cover the sentences the executive summary
//! actually promises.

use bioprism_evalengine::{
    attribute, compose, ArmSpec, Attribution, AttributionReport, CapabilityPosterior, Conclusion,
    Constraint, Contribution, CoverageFloor, CreditPolicy, EvalError, Justification, MatchedFork,
    Observation, Outcome, RefusalReason, ReleaseGate, ResultScore, Rubric, Satisfaction, ScoreTier,
    UnknownPolicy, Veto, VetoKind,
};
use std::collections::BTreeMap;

fn components(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn single(id: &str, tier: ScoreTier, conclusion: Conclusion) -> bioprism_evalengine::ScoredResult {
    compose(
        id,
        &[Contribution::new(tier, "evaluator@1", conclusion)],
        &UnknownPolicy::Block,
    )
    .expect("composes")
}

#[test]
fn a_judge_liking_a_run_the_deterministic_checker_failed_changes_nothing_downstream() {
    let observations: Vec<Observation> = (0..4)
        .map(|index| {
            let scored = compose(
                &format!("r{index}"),
                &[
                    Contribution::new(ScoreTier::Deterministic, "schema@1", Conclusion::Fail),
                    Contribution::new(ScoreTier::Judge, "rubric-judge@1", Conclusion::Pass),
                ],
                &UnknownPolicy::Block,
            )
            .expect("composes");
            Observation::new("planning", format!("parent-{index}"), scored)
        })
        .collect();

    let posterior =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    let planning = posterior.get("planning").expect("present");

    assert_eq!(planning.pass_rate.mean, 0.0);
    assert_eq!(planning.outcome_rate.mean, 0.0);
    assert_eq!(planning.optimistic_weak_evidence, 4);
    assert!(posterior.to_markdown().contains("more generous"));
}

#[test]
fn a_pack_of_unsupported_passes_reports_a_zero_pass_rate_and_a_visible_outcome_gap() {
    let observations: Vec<Observation> = (0..6)
        .map(|index| {
            Observation::new(
                "planning",
                format!("parent-{index}"),
                single(
                    &format!("r{index}"),
                    ScoreTier::Execution,
                    Conclusion::UnsupportedPass,
                ),
            )
        })
        .collect();

    let posterior =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    let planning = posterior.get("planning").expect("present");

    assert_eq!(planning.pass_rate.mean, 0.0);
    assert_eq!(planning.outcome_rate.mean, 1.0);
    assert_eq!(planning.unsupported_pass_gap(), 1.0);
    assert!(planning.credit.mean < 1.0);
}

#[test]
fn a_million_instances_from_three_parents_cannot_clear_an_effective_sample_floor() {
    let mut observations = Vec::new();
    for parent in 0..3 {
        for index in 0..400 {
            observations.push(Observation::new(
                "tool_use",
                format!("parent-{parent}"),
                single(
                    &format!("r{parent}-{index}"),
                    ScoreTier::Execution,
                    if parent == 0 {
                        Conclusion::Pass
                    } else {
                        Conclusion::Fail
                    },
                ),
            ));
        }
    }

    let posterior =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    let estimate = posterior.get("tool_use").expect("present");
    assert_eq!(estimate.pass_rate.instances, 1200);
    assert!(estimate.pass_rate.effective_sample_size < 10.0);

    let gate = ReleaseGate::new("ship", "the release checklist needs one number")
        .expect("rationale")
        .require("tool_use", CoverageFloor::requiring(3, 30.0));
    assert!(matches!(
        posterior.overall(&gate).unwrap_err(),
        EvalError::EffectiveSampleFloorUnmet { .. }
    ));
}

#[test]
fn a_scalar_is_unavailable_until_every_declared_floor_is_met() {
    let thin: Vec<Observation> = (0..2)
        .map(|index| {
            Observation::new(
                "planning",
                "parent-0",
                single(&format!("r{index}"), ScoreTier::Execution, Conclusion::Pass),
            )
        })
        .collect();
    let posterior = CapabilityPosterior::build(&thin, &CreditPolicy::default()).expect("builds");
    let gate = ReleaseGate::new("ship", "checklist")
        .expect("rationale")
        .require("planning", CoverageFloor::requiring(4, 4.0));
    assert!(posterior.overall(&gate).is_err());

    let broad: Vec<Observation> = (0..8)
        .map(|index| {
            Observation::new(
                "planning",
                format!("parent-{index}"),
                single(
                    &format!("r{index}"),
                    ScoreTier::Execution,
                    if index % 2 == 0 {
                        Conclusion::Pass
                    } else {
                        Conclusion::Fail
                    },
                ),
            )
        })
        .collect();
    let posterior = CapabilityPosterior::build(&broad, &CreditPolicy::default()).expect("builds");
    let scalar = posterior.overall(&gate).expect("floors met");
    assert_eq!(scalar.value, 0.5);
    assert_eq!(scalar.weakest_tier, ScoreTier::Execution);
    assert!(scalar.min_effective_sample >= 4.0);
}

#[test]
fn one_safety_veto_fails_a_gate_that_would_otherwise_pass_comfortably() {
    let mut observations: Vec<Observation> = (0..8)
        .map(|index| {
            Observation::new(
                "planning",
                format!("parent-{index}"),
                single(&format!("r{index}"), ScoreTier::Execution, Conclusion::Pass),
            )
        })
        .collect();

    let gate = ReleaseGate::new("ship", "checklist")
        .expect("rationale")
        .require("planning", CoverageFloor::requiring(4, 4.0));
    let clean =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    assert_eq!(clean.overall(&gate).expect("floors met").value, 1.0);

    observations.push(Observation::new(
        "planning",
        "parent-9",
        compose(
            "r-veto",
            &[Contribution::new(
                ScoreTier::Execution,
                "permission-check@1",
                Conclusion::Pass,
            )
            .with_veto(Veto::new(
                VetoKind::Permission,
                "permission-check@1",
                "wrote outside the sandbox",
            ))],
            &UnknownPolicy::Block,
        )
        .expect("composes"),
    ));

    let vetoed =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    assert!(matches!(
        vetoed.overall(&gate).unwrap_err(),
        EvalError::VetoOutstanding { .. }
    ));
    assert_eq!(vetoed.vetoed().count(), 1);
}

#[test]
fn a_fork_that_changed_two_things_at_once_yields_no_component_effect_anywhere_in_the_report() {
    let arm = |name: &str, pairs: &[(&str, &str)], conclusion: Conclusion| {
        ArmSpec::new(
            name,
            components(pairs),
            conclusion,
            ScoreTier::Deterministic,
        )
    };

    let report = AttributionReport::build(&[
        MatchedFork::new(
            "both-at-once",
            "cell-1",
            arm(
                "baseline",
                &[("model", "a"), ("planner", "p1")],
                Conclusion::Fail,
            ),
            arm(
                "variant",
                &[("model", "b"), ("planner", "p2")],
                Conclusion::Pass,
            ),
        )
        .controlled(),
    ]);

    assert!(report.effects.is_empty());
    assert_eq!(report.refusals().count(), 1);
    let markdown = report.to_markdown();
    assert!(markdown.contains("2 components varied at once"));
    assert!(!markdown.contains("| model |"));
}

#[test]
fn splitting_a_two_component_fork_into_two_forks_recovers_the_attribution() {
    let arm = |pairs: &[(&str, &str)], conclusion: Conclusion| {
        ArmSpec::new("arm", components(pairs), conclusion, ScoreTier::Execution)
    };

    let report = AttributionReport::build(&[
        MatchedFork::new(
            "vary-model",
            "cell-1",
            arm(&[("model", "a"), ("planner", "p1")], Conclusion::Fail),
            arm(&[("model", "b"), ("planner", "p1")], Conclusion::Fail),
        )
        .holding_fixed(["planner"])
        .controlled(),
        MatchedFork::new(
            "vary-planner",
            "cell-1",
            arm(&[("model", "a"), ("planner", "p1")], Conclusion::Fail),
            arm(&[("model", "a"), ("planner", "p2")], Conclusion::Pass),
        )
        .holding_fixed(["model"])
        .controlled(),
    ]);

    assert_eq!(report.effects.len(), 2);
    assert_eq!(report.refusals().count(), 0);
    let planner = report
        .effects
        .iter()
        .find(|effect| effect.component == "planner")
        .expect("present");
    assert_eq!(planner.improved, 1);
    assert!(planner.is_consistent());
    let model = report
        .effects
        .iter()
        .find(|effect| effect.component == "model")
        .expect("present");
    assert_eq!(model.unchanged, 1);
    assert_eq!(model.decisive(), 0);
}

#[test]
fn an_attribution_over_judge_scored_arms_is_labelled_judge_tier() {
    let fork = MatchedFork::new(
        "f1",
        "cell-1",
        ArmSpec::new(
            "baseline",
            components(&[("memory", "none")]),
            Conclusion::Fail,
            ScoreTier::Judge,
        ),
        ArmSpec::new(
            "variant",
            components(&[("memory", "episodic")]),
            Conclusion::Pass,
            ScoreTier::Deterministic,
        ),
    )
    .controlled();

    match attribute(&fork) {
        Attribution::Attributed {
            supporting_tier,
            component,
            ..
        } => {
            assert_eq!(supporting_tier, ScoreTier::Judge);
            assert_eq!(component, "memory");
        }
        other => panic!("expected an attribution, got {other:?}"),
    }
}

#[test]
fn a_fork_whose_control_broke_is_refused_even_when_only_one_component_changed() {
    let fork = MatchedFork::new(
        "f1",
        "cell-1",
        ArmSpec::new(
            "baseline",
            components(&[("budget", "10")]),
            Conclusion::Fail,
            ScoreTier::Execution,
        ),
        ArmSpec::new(
            "variant",
            components(&[("budget", "100")]),
            Conclusion::Pass,
            ScoreTier::Execution,
        ),
    )
    .holding_fixed(["budget"]);

    assert!(matches!(
        attribute(&fork),
        Attribution::Refused {
            reason: RefusalReason::HeldFixedViolated { .. }
        }
    ));
}

#[test]
fn rubric_derived_partial_credit_survives_composition_and_aggregation() {
    let rubric = Rubric::new(vec![
        Constraint::new("locate", 1, Satisfaction::Satisfied),
        Constraint::new("edit", 1, Satisfaction::Satisfied),
        Constraint::new("verify", 2, Satisfaction::Violated),
    ])
    .expect("distinct names");
    let score = ResultScore::new(Outcome::Incorrect, Justification::Supported)
        .with_rubric(rubric.clone());
    assert_eq!(score.conclusion(), Conclusion::PartialCredit);

    let observations: Vec<Observation> = (0..4)
        .map(|index| {
            let scored = compose(
                &format!("r{index}"),
                &[Contribution::new(
                    ScoreTier::Execution,
                    "runner@1",
                    Conclusion::PartialCredit,
                )
                .with_progress(rubric.progress())],
                &UnknownPolicy::Block,
            )
            .expect("composes");
            Observation::new("editing", format!("parent-{index}"), scored)
        })
        .collect();

    let posterior =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    let editing = posterior.get("editing").expect("present");
    assert_eq!(editing.pass_rate.mean, 0.0);
    assert!((editing.credit.mean - 0.5).abs() < 1e-9);
}

#[test]
fn a_disputed_result_never_reaches_a_pass_rate_denominator() {
    let disputed = compose(
        "r0",
        &[
            Contribution::new(ScoreTier::Execution, "runner-a@1", Conclusion::Pass),
            Contribution::new(ScoreTier::Execution, "runner-b@1", Conclusion::Fail),
        ],
        &UnknownPolicy::Block,
    )
    .expect("composes");
    assert!(disputed.needs_resolution());

    let observations = vec![
        Observation::new("planning", "parent-0", disputed),
        Observation::new(
            "planning",
            "parent-1",
            single("r1", ScoreTier::Execution, Conclusion::Pass),
        ),
    ];
    let posterior =
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
    let planning = posterior.get("planning").expect("present");

    assert_eq!(planning.pass_rate.mean, 1.0);
    assert_eq!(planning.pass_rate.unknown_instances, 1);
    assert_eq!(planning.disputed, 1);
}

#[test]
fn a_report_is_content_addressed_and_a_changed_conclusion_changes_the_digest() {
    let first = bioprism_evalengine::digest(&single(
        "r0",
        ScoreTier::Execution,
        Conclusion::Pass,
    ))
    .expect("hashable");
    let same = bioprism_evalengine::digest(&single(
        "r0",
        ScoreTier::Execution,
        Conclusion::Pass,
    ))
    .expect("hashable");
    let different = bioprism_evalengine::digest(&single(
        "r0",
        ScoreTier::Execution,
        Conclusion::Fail,
    ))
    .expect("hashable");

    assert_eq!(first, same);
    assert_ne!(first, different);
}

#[test]
fn observations_without_provenance_are_findable_before_publication() {
    let observations = vec![Observation::new(
        "planning",
        "parent-0",
        single("r0", ScoreTier::Execution, Conclusion::Pass),
    )];
    assert_eq!(bioprism_evalengine::unprovenanced(&observations).len(), 1);
}
