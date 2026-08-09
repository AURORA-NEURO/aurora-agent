//! Blueprint 23.21 and 23.22: compensation is not assumed to restore the prior state.

use bioprism_choreography::{
    ActionClass, Compensation, Failure, FailurePolicy, Recovery, RecoveryLadder, Remediation,
    Residual, Restoration, Saga, SagaError, SagaOutcome, Step, StepOutcome,
};
use std::collections::BTreeSet;

fn publish_release() -> Saga {
    Saga::new(
        "publish-release",
        vec![
            Step::new(
                "create-branch",
                ActionClass::ReversibleWrite,
                "key-branch",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::complete("delete-branch")),
            Step::new(
                "publish-package",
                ActionClass::CompensatableExternal,
                "key-package",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::partial(
                "yank-package-version",
                vec![Residual::new(
                    "consumers that already resolved the version keep it in their lockfiles",
                    Remediation::FollowUpRequired {
                        action: "publish an advisory for the yanked version".into(),
                    },
                )],
            )),
            Step::new(
                "notify-users",
                ActionClass::Irreversible,
                "key-notify",
                FailurePolicy::Escalate {
                    to: "release-manager".into(),
                },
            )
            .approved(),
        ],
    )
}

fn fully_reversible() -> Saga {
    Saga::new(
        "stage-artifacts",
        vec![
            Step::new(
                "create-branch",
                ActionClass::ReversibleWrite,
                "key-branch",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::complete("delete-branch")),
            Step::new(
                "stage-artifact",
                ActionClass::ReversibleWrite,
                "key-stage",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::complete("unstage-artifact")),
        ],
    )
}

#[test]
fn a_step_with_an_effect_and_no_compensation_does_not_validate() {
    let saga = Saga::new(
        "unsafe",
        vec![Step::new(
            "publish-package",
            ActionClass::CompensatableExternal,
            "key-1",
            FailurePolicy::Compensate,
        )],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::MissingCompensation {
            step: "publish-package".into(),
            class: "compensatable_external".into(),
        })
    );
}

#[test]
fn partial_restoration_must_name_what_it_could_not_undo() {
    let saga = Saga::new(
        "vague",
        vec![Step::new(
            "publish-package",
            ActionClass::CompensatableExternal,
            "key-1",
            FailurePolicy::Compensate,
        )
        .compensated_by(Compensation::partial("yank-package-version", vec![]))],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::UnnamedResidue {
            step: "publish-package".into()
        })
    );
}

#[test]
fn claiming_complete_restoration_while_listing_residue_does_not_validate() {
    let mut compensation = Compensation::complete("delete-branch");
    compensation.residuals.push(Residual::new(
        "the branch name is still reserved",
        Remediation::NotRequired,
    ));
    let saga = Saga::new(
        "contradictory",
        vec![Step::new(
            "create-branch",
            ActionClass::ReversibleWrite,
            "key-1",
            FailurePolicy::Compensate,
        )
        .compensated_by(compensation)],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::ResidueOnCompleteRestoration {
            step: "create-branch".into()
        })
    );
}

#[test]
fn an_irreversible_step_must_require_approval_before_execution() {
    let saga = Saga::new(
        "reckless",
        vec![Step::new(
            "notify-users",
            ActionClass::Irreversible,
            "key-1",
            FailurePolicy::Abort,
        )],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::UnapprovedIrreversibleStep {
            step: "notify-users".into()
        })
    );
}

#[test]
fn an_irreversible_step_may_not_claim_a_compensation() {
    let saga = Saga::new(
        "wishful",
        vec![Step::new(
            "notify-users",
            ActionClass::Irreversible,
            "key-1",
            FailurePolicy::Abort,
        )
        .approved()
        .compensated_by(Compensation::complete("unsend-email"))],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::CompensationOnIrreversibleStep {
            step: "notify-users".into(),
            compensation: "unsend-email".into(),
        })
    );
}

#[test]
fn two_steps_sharing_an_idempotency_key_do_not_validate() {
    let saga = Saga::new(
        "collided",
        vec![
            Step::new(
                "charge-account",
                ActionClass::IdempotentWrite,
                "key-1",
                FailurePolicy::Retry,
            ),
            Step::new(
                "charge-fee",
                ActionClass::IdempotentWrite,
                "key-1",
                FailurePolicy::Retry,
            ),
        ],
    );

    assert!(matches!(
        saga.validate(),
        Err(SagaError::DuplicateIdempotencyKey { .. })
    ));
}

#[test]
fn a_side_effecting_step_without_an_idempotency_key_does_not_validate() {
    let saga = Saga::new(
        "unkeyed",
        vec![Step::new(
            "create-branch",
            ActionClass::ReversibleWrite,
            "",
            FailurePolicy::Compensate,
        )
        .compensated_by(Compensation::complete("delete-branch"))],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::MissingIdempotencyKey {
            step: "create-branch".into()
        })
    );
}

#[test]
fn a_retry_policy_on_a_non_idempotent_step_does_not_validate() {
    let saga = Saga::new(
        "unsafe-retry",
        vec![Step::new(
            "publish-package",
            ActionClass::CompensatableExternal,
            "key-1",
            FailurePolicy::Retry,
        )
        .compensated_by(Compensation::complete("yank-package-version"))],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::RetryOnNonIdempotentStep {
            step: "publish-package".into(),
            class: "compensatable_external".into(),
        })
    );
}

#[test]
fn a_compensation_needing_authority_the_saga_does_not_hold_is_unreachable() {
    let saga = Saga::new(
        "ungranted",
        vec![Step::new(
            "publish-package",
            ActionClass::CompensatableExternal,
            "key-1",
            FailurePolicy::Compensate,
        )
        .compensated_by(
            Compensation::complete("yank-package-version").needing_authority("registry:write"),
        )],
    );

    assert_eq!(
        saga.validate(),
        Err(SagaError::UnreachableCompensation {
            step: "publish-package".into(),
            authority: "registry:write".into(),
        })
    );
}

#[test]
fn every_problem_is_reported_rather_than_only_the_first() {
    let saga = Saga::new(
        "several",
        vec![
            Step::new("a", ActionClass::ReversibleWrite, "", FailurePolicy::Retry),
            Step::new(
                "b",
                ActionClass::Irreversible,
                "key-b",
                FailurePolicy::Abort,
            ),
        ],
    );

    let problems = saga.problems();
    assert!(problems.len() >= 4, "got {problems:?}");
}

#[test]
fn compensation_with_residue_is_not_reported_as_a_rollback() {
    let saga = publish_release();
    let outcome = saga
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::InvariantViolated,
                    detail: "the release notes contradict the changelog".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("the saga validates");

    let SagaOutcome::CompensatedWithResidue { residue, .. } = &outcome else {
        panic!("expected residue to be reported, got {outcome:?}");
    };
    assert_eq!(residue.len(), 1);
    assert!(residue[0].description.contains("lockfiles"));
    assert!(
        !outcome.restored_prior_state(),
        "yanking a version does not unpublish it from anybody's cache"
    );
    assert_eq!(outcome.outstanding_remediation().len(), 1);
}

#[test]
fn full_compensation_is_the_only_outcome_that_claims_the_prior_state() {
    let outcome = fully_reversible()
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::InvariantViolated,
                    detail: "the artifact digest does not match".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    assert!(matches!(outcome, SagaOutcome::CompensatedFully { .. }));
    assert!(outcome.restored_prior_state());
    assert!(outcome.residue().is_empty());
}

#[test]
fn a_committed_saga_reports_every_step_and_claims_no_rollback() {
    let outcome = fully_reversible()
        .run_with_declared_authority(
            &[StepOutcome::Succeeded, StepOutcome::Succeeded],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    assert!(outcome.committed());
    assert!(!outcome.restored_prior_state());
    assert_eq!(outcome.steps().len(), 2);
    assert!(outcome.steps().iter().all(|step| step.succeeded));
}

#[test]
fn authority_revoked_between_prepare_and_commit_blocks_compensation() {
    let saga = Saga::new(
        "escrowed",
        vec![
            Step::new(
                "publish-package",
                ActionClass::CompensatableExternal,
                "key-package",
                FailurePolicy::Compensate,
            )
            .compensated_by(
                Compensation::complete("yank-package-version").needing_authority("registry:write"),
            ),
            Step::new(
                "create-release-notes",
                ActionClass::ReversibleWrite,
                "key-notes",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::complete("mark-release-withdrawn")),
        ],
    )
    .holding("registry:write");

    saga.validate().expect("the plan is coherent at design time");

    let outcome = saga
        .run(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::InvariantViolated,
                    detail: "the notes reference an unreleased commit".into(),
                },
            ],
            &BTreeSet::new(),
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let SagaOutcome::CompensationBlocked {
        blocked_at, reason, ..
    } = &outcome
    else {
        panic!("expected a blocked compensation, got {outcome:?}");
    };
    assert_eq!(blocked_at, "publish-package");
    assert!(reason.contains("registry:write"));
    assert!(!outcome.restored_prior_state());
}

#[test]
fn an_unobserved_step_is_compensated_and_says_the_effect_was_never_confirmed() {
    let outcome = fully_reversible()
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Unobserved {
                    detail: "the staging call timed out after the request was accepted".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let SagaOutcome::CompensatedWithResidue { residue, steps, .. } = &outcome else {
        panic!("an unconfirmed effect cannot be reported as a clean rollback, got {outcome:?}");
    };
    assert!(residue
        .iter()
        .any(|residual| residual.description.contains("never confirmed")));
    assert!(steps.iter().any(|step| !step.observed));
    assert!(!outcome.restored_prior_state());
}

#[test]
fn retry_is_not_selected_for_a_non_idempotent_step() {
    let ladder = RecoveryLadder::default();
    let step = Step::new(
        "publish-package",
        ActionClass::CompensatableExternal,
        "key-1",
        FailurePolicy::Retry,
    );

    let recovery = ladder.select(&step, Failure::Timeout);
    assert!(
        matches!(recovery, Recovery::Compensate { .. }),
        "repeating a non-idempotent external action could duplicate the effect, got {recovery:?}"
    );
}

#[test]
fn retry_is_selected_for_an_idempotent_step_under_a_transient_failure() {
    let ladder = RecoveryLadder::default();
    let step = Step::new(
        "reserve-inventory",
        ActionClass::IdempotentWrite,
        "key-1",
        FailurePolicy::Retry,
    );

    assert!(matches!(
        ladder.select(&step, Failure::Timeout),
        Recovery::Retry { .. }
    ));
    assert!(matches!(
        ladder.select(&step, Failure::StaleVersion),
        Recovery::Retry { .. }
    ));
}

#[test]
fn an_invariant_violation_unwinds_regardless_of_the_declared_policy() {
    let ladder = RecoveryLadder::default();
    let step = Step::new(
        "reserve-inventory",
        ActionClass::IdempotentWrite,
        "key-1",
        FailurePolicy::Retry,
    );

    let recovery = ladder.select(&step, Failure::InvariantViolated);
    assert!(matches!(recovery, Recovery::Compensate { .. }));
    assert!(recovery.unwinds());
}

#[test]
fn a_policy_denial_escalates_rather_than_retrying() {
    let ladder = RecoveryLadder::new("safety-officer");
    let step = Step::new(
        "reserve-inventory",
        ActionClass::IdempotentWrite,
        "key-1",
        FailurePolicy::Retry,
    );

    let Recovery::Escalate { to, .. } = ladder.select(&step, Failure::PolicyDenied) else {
        panic!("a policy denial is not a transient failure");
    };
    assert_eq!(to, "safety-officer");
}

#[test]
fn a_malformed_response_rebinds_rather_than_repeating_the_same_request() {
    let ladder = RecoveryLadder::default();
    let step = Step::new(
        "summarise-findings",
        ActionClass::ReadOnly,
        "key-1",
        FailurePolicy::Retry,
    );

    assert!(matches!(
        ladder.select(&step, Failure::MalformedResponse),
        Recovery::Rebind { .. }
    ));
}

#[test]
fn a_suspended_saga_names_the_rung_and_leaves_the_effects_in_place() {
    let outcome = publish_release()
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::ToolUnavailable,
                    detail: "the mail relay is down".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let SagaOutcome::Suspended { at, recovery, .. } = &outcome else {
        panic!("expected suspension, got {outcome:?}");
    };
    assert_eq!(at, "notify-users");
    assert!(matches!(recovery, Recovery::Escalate { .. }));
    assert!(!outcome.restored_prior_state());
    assert!(outcome.residue().is_empty());
}

#[test]
fn an_accepted_partial_result_is_still_reported_as_partial() {
    let saga = Saga::new(
        "gather-observations",
        vec![
            Step::new(
                "collect-primary",
                ActionClass::ReversibleWrite,
                "key-primary",
                FailurePolicy::Compensate,
            )
            .compensated_by(Compensation::partial(
                "discard-primary",
                vec![Residual::new(
                    "downstream caches already saw the primary observation",
                    Remediation::HumanNotification {
                        audience: "data steward".into(),
                    },
                )],
            )),
            Step::new(
                "collect-secondary",
                ActionClass::ReadOnly,
                "key-secondary",
                FailurePolicy::AcceptPartial,
            ),
        ],
    );

    let outcome = saga
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::ToolUnavailable,
                    detail: "the secondary source is offline".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let SagaOutcome::PartiallyAccepted { stopped_at, .. } = &outcome else {
        panic!("expected a declared partial result, got {outcome:?}");
    };
    assert_eq!(stopped_at, "collect-secondary");
    assert!(!outcome.restored_prior_state());
    assert!(!outcome.residue().is_empty());
}

#[test]
fn an_outcome_script_shorter_than_the_saga_is_an_error_rather_than_a_success() {
    let error = fully_reversible()
        .run_with_declared_authority(&[StepOutcome::Succeeded], &RecoveryLadder::default())
        .expect_err("there is no recorded result for the second step");

    assert_eq!(
        error,
        SagaError::OutcomeScriptTooShort {
            step: "stage-artifact".into()
        }
    );
}

#[test]
fn an_invalid_saga_never_runs() {
    let saga = Saga::new(
        "invalid",
        vec![Step::new(
            "publish-package",
            ActionClass::CompensatableExternal,
            "key-1",
            FailurePolicy::Compensate,
        )],
    );

    assert!(saga
        .run_with_declared_authority(&[StepOutcome::Succeeded], &RecoveryLadder::default())
        .is_err());
}

#[test]
fn compensation_runs_in_reverse_order_of_the_steps_that_completed() {
    let outcome = fully_reversible()
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::InvariantViolated,
                    detail: "digest mismatch".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let compensated: Vec<&str> = outcome
        .steps()
        .iter()
        .filter(|step| {
            step.compensation
                .as_ref()
                .is_some_and(|compensation| compensation.executed)
        })
        .map(|step| step.step.as_str())
        .collect();
    assert_eq!(
        compensated,
        vec!["create-branch"],
        "only the step that actually completed is undone"
    );
}

#[test]
fn a_saga_outcome_round_trips_without_losing_the_residue() {
    let outcome = publish_release()
        .run_with_declared_authority(
            &[
                StepOutcome::Succeeded,
                StepOutcome::Succeeded,
                StepOutcome::Failed {
                    failure: Failure::InvariantViolated,
                    detail: "contradictory notes".into(),
                },
            ],
            &RecoveryLadder::default(),
        )
        .expect("validates");

    let encoded = serde_json::to_string(&outcome).expect("serialises");
    assert!(encoded.contains("compensated_with_residue"));
    let decoded: SagaOutcome = serde_json::from_str(&encoded).expect("round trips");
    assert!(!decoded.restored_prior_state());
    assert_eq!(decoded.residue().len(), 1);
}

#[test]
fn restoration_levels_are_distinguishable_after_serialisation() {
    let complete = serde_json::to_string(&Restoration::Complete).expect("serialises");
    let partial = serde_json::to_string(&Restoration::Partial).expect("serialises");
    assert_ne!(complete, partial);
}
