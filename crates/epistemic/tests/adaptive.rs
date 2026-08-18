use bioprism_epistemic::{
    adaptive_policy, Acquisition, AdaptiveNode, Belief, DecisionProblem, EpistemicError,
};

fn binary_problem() -> DecisionProblem {
    DecisionProblem::new(
        vec!["choose-m0".into(), "choose-m1".into()],
        vec!["m0".into(), "m1".into()],
        vec![0.0, 1.0, 1.0, 0.0],
    )
    .unwrap()
}

#[test]
fn adaptive_policy_changes_the_next_acquisition_by_observed_outcome() {
    let problem = binary_problem();
    let belief = Belief::new(vec![0.9, 0.1]).unwrap();
    let acquisitions = vec![
        Acquisition::binary("screen", 0.01, vec![0.9, 0.2]).unwrap(),
        Acquisition::binary("confirm", 0.1, vec![0.01, 0.99]).unwrap(),
    ];

    let policy = adaptive_policy(&problem, &belief, &acquisitions, 0.11, 2).unwrap();
    assert!(policy.expected_total < problem.bayes_risk(&belief));
    assert!(policy.expected_acquisition_cost >= 0.01);
    assert_eq!(policy.selected_depth, 2);
    let AdaptiveNode::Acquire { id, outcomes, .. } = policy.root else {
        panic!("the informative first acquisition should be selected");
    };
    assert_eq!(id, "screen");
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().any(|outcome| matches!(outcome.next.as_ref(), AdaptiveNode::Acquire { id, .. } if id == "confirm")));
    assert!(outcomes
        .iter()
        .any(|outcome| matches!(outcome.next.as_ref(), AdaptiveNode::Stop { .. })));
}

#[test]
fn adaptive_policy_prefers_stop_when_cost_exceeds_expected_reduction() {
    let problem = binary_problem();
    let belief = Belief::new(vec![0.5, 0.5]).unwrap();
    let acquisition = Acquisition::binary("too-expensive", 1.0, vec![0.99, 0.01]).unwrap();
    let policy = adaptive_policy(&problem, &belief, &[acquisition], 1.0, 1).unwrap();
    assert_eq!(policy.expected_total, 0.5);
    assert!(
        matches!(policy.root, AdaptiveNode::Stop { action: 0, risk } if (risk - 0.5).abs() < 1e-12)
    );
}

#[test]
fn adaptive_policy_refuses_unbounded_requests_and_malformed_serde_inputs() {
    let problem = binary_problem();
    let belief = Belief::uniform(2).unwrap();
    let acquisition = Acquisition::binary("test", 0.1, vec![0.9, 0.1]).unwrap();
    assert!(matches!(
        adaptive_policy(
            &problem,
            &belief,
            std::slice::from_ref(&acquisition),
            1.0,
            17
        ),
        Err(EpistemicError::AdaptiveStepLimit { .. })
    ));
    let mut malformed = serde_json::to_value(acquisition).unwrap();
    malformed["outcomes"][0]["likelihood"] = serde_json::json!([0.5]);
    let parsed: Acquisition = serde_json::from_value(malformed).unwrap();
    assert!(matches!(
        adaptive_policy(&problem, &belief, &[parsed], 1.0, 1),
        Err(EpistemicError::LikelihoodShape { .. })
    ));
}
