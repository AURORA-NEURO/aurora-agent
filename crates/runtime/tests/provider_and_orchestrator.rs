//! Executor provider interface (05.03) and run orchestrator (05.02).
//!
//! The provider tests are mostly about what the runtime refuses to do: an unimplemented backend
//! errors instead of degrading, and a plan that needs a capability nobody has is rejected before it
//! runs rather than after it produces a mislabelled result. The orchestrator tests are about
//! finalizing honestly — idempotently, and naming the evidence a cancellation cost.

use bioprism_ids::RunId;
use bioprism_runtime::{
    AttemptId, Capabilities, ContainerProvider, EffectKind, EffectPolicy, ExecutionPlan,
    ExecutorProvider, InProcessProvider, InProcessWorld, RetryClass, RunState, RuntimeError,
    Sandbox, SubprocessProvider, TerminationReason, Trial, TrialId,
};

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn trial(id: &str) -> TrialId {
    TrialId::parse(id).expect("well-formed trial id")
}

fn attempt(id: &str) -> AttemptId {
    AttemptId::parse(id).expect("well-formed attempt id")
}

fn plan(id: &str) -> ExecutionPlan {
    ExecutionPlan::new(run("run-1"), trial(id), attempt("attempt-1")).with_policy(
        EffectPolicy::evaluation_default()
            .declaring([EffectKind::FileRead, EffectKind::FileWrite])
            .allowing_path("/work/"),
    )
}

#[test]
fn an_unavailable_provider_refuses_rather_than_degrading() {
    let mut subprocess = SubprocessProvider;
    let mut container = ContainerProvider;
    let plan = plan("trial-1");

    for (name, error) in [
        ("subprocess", subprocess.prepare(&plan).expect_err("declared only")),
        ("container", container.prepare(&plan).expect_err("declared only")),
    ] {
        match error {
            RuntimeError::ProviderUnavailable {
                provider,
                operation,
            } => {
                assert_eq!(provider, name);
                assert_eq!(operation, "prepare");
            }
            other => panic!("expected provider unavailable, got {other}"),
        }
    }
}

#[test]
fn an_unavailable_provider_advertises_no_capabilities_so_nothing_selects_it() {
    let subprocess = SubprocessProvider;
    let container = ContainerProvider;

    assert!(!subprocess.is_available());
    assert!(!container.is_available());
    assert_eq!(subprocess.capabilities(), Capabilities::none());
    assert_eq!(container.capabilities(), Capabilities::none());
    assert!(InProcessProvider::new().is_available());
}

#[test]
fn the_in_process_provider_declares_only_the_capabilities_it_has() {
    let capabilities = InProcessProvider::new().capabilities();

    assert!(!capabilities.container_isolation, "there is no container here");
    assert!(!capabilities.process_isolation, "there is no process boundary here");
    assert!(!capabilities.process_checkpoints);
    assert!(capabilities.nested_forks, "forking a tape needs no provider help");
    assert!(capabilities.network_fixtures);
}

#[test]
fn a_plan_that_needs_a_capability_the_provider_lacks_is_refused_before_it_runs() {
    let mut provider = InProcessProvider::new();
    let demanding = plan("trial-iso").requiring(Capabilities {
        container_isolation: true,
        ..Capabilities::none()
    });

    let error = provider
        .prepare(&demanding)
        .expect_err("a thread is not a container");
    match error {
        RuntimeError::CapabilityUnsupported { capability, .. } => {
            assert_eq!(capability, "container_isolation");
        }
        other => panic!("expected an unsupported capability, got {other}"),
    }
}

#[test]
fn a_prepared_trial_must_be_started_before_it_can_record() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-2")).expect("supported");

    provider
        .open(&handle, InProcessWorld::new())
        .expect_err("preparing is not starting");
    provider.start(&handle).expect("the handle is known");
    provider
        .open(&handle, InProcessWorld::new())
        .expect("a started trial can record");
}

#[test]
fn a_provider_refuses_a_handle_it_never_issued() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-3")).expect("supported");
    provider.destroy(&handle).expect("cleanup succeeds");

    let error = provider
        .start(&handle)
        .expect_err("a destroyed trial is not a trial");
    assert!(matches!(error, RuntimeError::UnknownHandle { .. }));
}

#[test]
fn a_checkpoint_commits_to_the_tape_head_it_was_taken_from() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-4")).expect("supported");
    provider.start(&handle).expect("known");

    let mut host = provider
        .open(&handle, InProcessWorld::new().with_base_file("/work/in.txt", "x"))
        .expect("started");
    host.read_file("/work/in.txt").expect("declared and allowed");
    host.write_file("/work/out.txt", "written").expect("allowed");
    let tape = host.into_tape();
    let head = tape.head().to_string();
    provider.commit(&handle, tape).expect("known");

    let checkpoint = provider.checkpoint(&handle).expect("known");
    assert_eq!(checkpoint.step, 2);
    assert_eq!(checkpoint.tape_head, head);
    assert!(checkpoint.restoration.portable);

    let resumed = provider.resume(&checkpoint).expect("its own checkpoint");
    assert_eq!(resumed.commitment, head);
}

#[test]
fn resuming_a_checkpoint_no_trial_holds_is_refused() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-5")).expect("supported");
    provider.start(&handle).expect("known");
    let checkpoint = provider.checkpoint(&handle).expect("known");
    provider.destroy(&handle).expect("cleanup");

    let error = provider
        .resume(&checkpoint)
        .expect_err("nothing holds this checkpoint any more");
    assert!(matches!(error, RuntimeError::CorruptCheckpoint { .. }));
}

#[test]
fn collect_reports_what_was_created_and_what_was_only_read() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-6")).expect("supported");
    provider.start(&handle).expect("known");

    let mut host = provider
        .open(&handle, InProcessWorld::new().with_base_file("/work/in.txt", "x"))
        .expect("started");
    host.read_file("/work/in.txt").expect("allowed");
    host.write_file("/work/out.txt", "written").expect("allowed");
    let tape = host.into_tape();
    provider.commit(&handle, tape).expect("known");

    let artifacts = provider.collect(&handle).expect("known");
    let created: Vec<&str> = artifacts
        .iter()
        .filter(|artifact| artifact.created)
        .map(|artifact| artifact.path.as_str())
        .collect();
    let read_only: Vec<&str> = artifacts
        .iter()
        .filter(|artifact| !artifact.created)
        .map(|artifact| artifact.path.as_str())
        .collect();

    assert_eq!(created, vec!["/work/out.txt"]);
    assert_eq!(read_only, vec!["/work/in.txt"]);
    assert_eq!(artifacts[1].bytes, "written".len() as u64);
}

#[test]
fn the_event_stream_resumes_from_a_cursor() {
    let mut provider = InProcessProvider::new();
    let handle = provider.prepare(&plan("trial-7")).expect("supported");
    provider.start(&handle).expect("known");

    let mut host = provider
        .open(&handle, InProcessWorld::new().with_base_file("/work/in.txt", "x"))
        .expect("started");
    host.read_file("/work/in.txt").expect("allowed");
    host.write_file("/work/a.txt", "a").expect("allowed");
    host.write_file("/work/b.txt", "b").expect("allowed");
    let tape = host.into_tape();
    provider.commit(&handle, tape).expect("known");

    assert_eq!(provider.events(&handle, 0).expect("known").len(), 3);
    assert_eq!(provider.events(&handle, 2).expect("known")[0].step, 2);
    assert!(provider.events(&handle, 3).expect("the end is a valid cursor").is_empty());
    assert!(provider.events(&handle, 4).is_err());
}

#[test]
fn an_execution_plan_digest_changes_when_the_plan_changes() {
    let base = plan("trial-8");
    let reseeded = plan("trial-8").with_seed(9);

    assert_ne!(
        base.digest().expect("serializes"),
        reseeded.digest().expect("serializes"),
        "a frozen manifest that ignores the seed is not frozen"
    );
    assert_eq!(
        base.digest().expect("serializes"),
        plan("trial-8").digest().expect("serializes")
    );
}

#[test]
fn an_illegal_lifecycle_transition_is_refused() {
    let mut subject = Trial::new(run("run-1"), trial("trial-9"), attempt("attempt-1"));

    let error = subject
        .advance(RunState::Completed)
        .expect_err("a queued trial has not been evaluated");
    match error {
        RuntimeError::IllegalTransition { from, to } => {
            assert_eq!(from, RunState::Queued);
            assert_eq!(to, RunState::Completed);
        }
        other => panic!("expected an illegal transition, got {other}"),
    }
    assert_eq!(subject.state(), RunState::Queued);
}

#[test]
fn every_transition_emits_exactly_one_event() {
    let mut subject = Trial::new(run("run-1"), trial("trial-10"), attempt("attempt-1"));
    subject.dispatch().expect("the happy path is legal");

    assert_eq!(subject.state(), RunState::Running);
    let states: Vec<RunState> = subject.events().iter().map(|event| event.to).collect();
    assert_eq!(
        states,
        vec![RunState::Leased, RunState::Preparing, RunState::Running]
    );
    assert_eq!(subject.events()[2].seq, 2);
}

#[test]
fn finalizing_twice_is_idempotent_and_does_not_duplicate_events() {
    let mut subject = Trial::new(run("run-1"), trial("trial-11"), attempt("attempt-1"));
    subject.dispatch().expect("legal");
    subject.advance(RunState::Evaluating).expect("legal");
    subject.advance(RunState::Finalizing).expect("legal");

    let first = subject
        .finalize(TerminationReason::Completed, Vec::new())
        .expect("legal")
        .clone();
    let events_after_first = subject.events().len();

    let second = subject
        .finalize(
            TerminationReason::TaskFailure {
                detail: "a failed-over controller re-finalizing".into(),
            },
            vec!["everything".into()],
        )
        .expect("a second finalize is not an error")
        .clone();

    assert_eq!(
        second, first,
        "the run itself is a better witness than a controller that lost its lease"
    );
    assert_eq!(subject.events().len(), events_after_first);
    assert_eq!(subject.state(), RunState::Completed);
}

#[test]
fn a_forced_cancellation_records_which_evidence_is_missing() {
    let mut subject = Trial::new(run("run-1"), trial("trial-12"), attempt("attempt-1"));
    subject.dispatch().expect("legal");
    subject.set_task_millis(4_000);

    let termination = subject
        .cancel(true, vec!["evaluator output".into(), "final artifacts".into()])
        .expect("cancellation from running is legal")
        .clone();

    assert_eq!(subject.state(), RunState::Cancelled);
    assert_eq!(
        termination.reason,
        TerminationReason::Cancelled { forced: true }
    );
    assert_eq!(termination.missing_evidence.len(), 2);
    assert_eq!(termination.task_millis, 4_000);
}

#[test]
fn a_graceful_cancellation_can_state_that_nothing_was_lost() {
    let mut subject = Trial::new(run("run-1"), trial("trial-13"), attempt("attempt-1"));
    subject.dispatch().expect("legal");

    let termination = subject.cancel(false, Vec::new()).expect("legal").clone();
    assert_eq!(
        termination.reason,
        TerminationReason::Cancelled { forced: false }
    );
    assert!(
        termination.missing_evidence.is_empty(),
        "completeness is stated on the record, not assumed by its reader"
    );
}

#[test]
fn a_budget_failure_terminates_the_trial_as_a_budget_failure_not_a_task_failure() {
    let mut subject = Trial::new(run("run-1"), trial("trial-14"), attempt("attempt-1"));
    subject.dispatch().expect("legal");

    let termination = subject
        .finalize(
            TerminationReason::BudgetExhausted {
                resource: bioprism_runtime::RuntimeResource::ModelTokens,
            },
            vec!["the remainder of the task".into()],
        )
        .expect("legal")
        .clone();

    assert_eq!(subject.state(), RunState::Failed);
    assert!(matches!(
        termination.reason,
        TerminationReason::BudgetExhausted { .. }
    ));
}

#[test]
fn a_retry_gets_a_new_attempt_id_and_keeps_the_prior_attempts() {
    let mut first = Trial::new(run("run-1"), trial("trial-15"), attempt("attempt-1"));
    first.dispatch().expect("legal");
    first
        .finalize(
            TerminationReason::ProviderUnavailable {
                provider: "container".into(),
            },
            Vec::new(),
        )
        .expect("legal");

    let second = first
        .retry(attempt("attempt-2"), RetryClass::Infrastructure, "deadbeef")
        .expect("a terminal trial can be retried");

    assert_eq!(second.id(), first.id(), "the trial identity does not change");
    assert_eq!(second.attempt().as_str(), "attempt-2");
    assert_eq!(second.state(), RunState::Queued);
    assert_eq!(second.prior_attempts().len(), 1);
    assert_eq!(second.prior_attempts()[0].attempt.as_str(), "attempt-1");
    assert_eq!(second.prior_attempts()[0].tape_head, "deadbeef");
    assert_eq!(
        second.prior_attempts()[0].retry_class,
        RetryClass::Infrastructure
    );
}

#[test]
fn a_running_trial_cannot_be_retried_out_from_under_itself() {
    let mut subject = Trial::new(run("run-1"), trial("trial-16"), attempt("attempt-1"));
    subject.dispatch().expect("legal");

    let error = subject
        .retry(attempt("attempt-2"), RetryClass::Agent, "")
        .expect_err("the first attempt has not finished");
    assert!(matches!(error, RuntimeError::RunNotRunnable { .. }));
}

#[test]
fn an_empty_identifier_is_refused_rather_than_silently_accepted() {
    assert!(matches!(
        TrialId::parse(""),
        Err(RuntimeError::MalformedId { kind: "trial", .. })
    ));
    assert!(matches!(
        AttemptId::parse("attempt\u{0}1"),
        Err(RuntimeError::MalformedId {
            kind: "attempt",
            ..
        })
    ));
}
