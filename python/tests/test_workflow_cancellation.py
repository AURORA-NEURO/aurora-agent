"""Cooperative stop requests preserve observations and never confer dispatch/retry authority."""
from copy import deepcopy
from threading import Event, Thread

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.workflow_builder import (
    MAX_WORKFLOW_ATTEMPTS, WORKFLOW_CHECKPOINT_SCHEMA, WORKFLOW_CHECKPOINT_SCHEMA_V01,
    WorkflowCancellationToken, WorkflowValidationError, compile_workflow_plan,
    execute_workflow_plan, load_workflow_checkpoint, save_workflow_checkpoint,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import compile_workflow_requirements


def _case(executor=None, *, one_step=False, requirements=None):
    catalog, calls = WorkflowOperationCatalog(), []
    schema = {"type": "object", "properties": {"text": {"type": "string"}},
              "required": ["text"], "additionalProperties": False}

    def call(arguments, **kwargs):
        calls.append(arguments["text"])
        if executor is not None:
            executor(arguments)
        return WorkflowOperationResult({"text": arguments["text"].upper()}, {"fixture": "actual_completed_call"})

    catalog.register(WorkflowOperation("fixture.echo", "Reviewed fixture read", schema, schema,
                                       "fixture", implementation="live_adapter"), call)
    steps = [{"id": "first", "operation": "fixture.echo", "arguments": {"text": "first"}, "depends_on": []}]
    if not one_step:
        steps.append({"id": "second", "operation": "fixture.echo", "arguments": {"text": {"$ref": "first#/text"}}, "depends_on": ["first"]})
    plan = compile_workflow_plan({"name": "Cancellation fixture", "steps": steps, "outputs": [steps[-1]["id"]]},
                                catalog, task="Read fixture text", requirements=requirements)
    return catalog, calls, plan


def _restamp(value):
    value["checkpoint_digest"] = content_digest({key: child for key, child in value.items() if key != "checkpoint_digest"})
    return value


def test_requested_token_is_one_way_and_stops_even_before_source_approval(tmp_path):
    catalog, calls, plan = _case()
    token = WorkflowCancellationToken()
    assert token.cancellation_requested is False
    token.cancel()
    token.cancel()
    path = tmp_path / "cancelled.json"
    run = execute_workflow_plan(plan, catalog, cancellation=token,
                                on_checkpoint=lambda value: save_workflow_checkpoint(path, value))
    assert token.cancellation_requested is True
    assert run.status == "cancelled"
    assert calls == []
    assert run.outputs == {}
    assert run.checkpoint["attempts"] == {}
    assert run.checkpoint["failure"] is run.checkpoint["in_flight"] is None
    assert load_workflow_checkpoint(path) == run.checkpoint
    assert run.checkpoint["schema"] == WORKFLOW_CHECKPOINT_SCHEMA
    assert run.checkpoint["cancel_requested"] is True


def test_between_step_cancellation_is_durable_and_does_not_repeat_completed_values(tmp_path):
    catalog, calls, plan = _case()
    token = WorkflowCancellationToken()
    path = tmp_path / "cancelled.json"

    def persist(value):
        save_workflow_checkpoint(path, value)
        if "first" in value["results"]:
            token.cancel()

    run = execute_workflow_plan(plan, catalog, approved=True, cancellation=token, on_checkpoint=persist)
    assert run.status == "cancelled"
    assert calls == ["first"]
    assert run.checkpoint["attempts"] == {"first": 1}
    assert run.checkpoint["results"]["first"]["value"] == {"text": "FIRST"}
    assert run.checkpoint["results"]["first"]["receipt"] == {"fixture": "actual_completed_call"}
    stored = load_workflow_checkpoint(path)
    assert stored == run.checkpoint
    held = execute_workflow_plan(plan, catalog, approved=True, retry_failed=True, checkpoint=stored)
    assert held.status == "cancelled"
    assert calls == ["first"]
    still_requested = execute_workflow_plan(plan, catalog, approved=True, checkpoint=stored,
        cancellation=token, resume_cancelled=True)
    assert still_requested.status == "cancelled"
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=stored,
        cancellation=WorkflowCancellationToken(), resume_cancelled=True)
    assert resumed.status == "completed"
    assert resumed.checkpoint["cancel_requested"] is False
    assert resumed.outputs == {"second": {"text": "FIRST"}}
    assert calls == ["first", "FIRST"]


def test_in_flight_synchronous_call_drains_and_is_retained_before_cancellation(tmp_path):
    entered, release = Event(), Event()
    token = WorkflowCancellationToken()

    def block(_arguments):
        entered.set()
        assert release.wait(5), "test did not release the admitted fixture call"

    catalog, calls, plan = _case(block)
    outcomes, errors = [], []
    path = tmp_path / "inflight.json"

    def work():
        try:
            outcomes.append(execute_workflow_plan(plan, catalog, approved=True, cancellation=token,
                on_checkpoint=lambda value: save_workflow_checkpoint(path, value)))
        except BaseException as error:
            errors.append(error)

    thread = Thread(target=work)
    thread.start()
    try:
        assert entered.wait(5)
        token.cancel()
        assert thread.is_alive(), "cancellation must not detach the synchronous call"
        assert calls == ["first"]
        assert load_workflow_checkpoint(path)["in_flight"] == "first"
    finally:
        release.set()
        thread.join(5)
    assert not thread.is_alive()
    assert errors == []
    run = outcomes[0]
    assert run.status == "cancelled"
    assert calls == ["first"]
    assert run.checkpoint["in_flight"] is None
    assert run.checkpoint["results"]["first"]["value"] == {"text": "FIRST"}
    assert load_workflow_checkpoint(path) == run.checkpoint


def test_cancelled_final_call_retains_final_output_without_claiming_uncancelled_completion():
    token = WorkflowCancellationToken()
    catalog, calls, plan = _case(lambda _: token.cancel(), one_step=True)
    run = execute_workflow_plan(plan, catalog, approved=True, cancellation=token)
    assert run.status == "cancelled"
    assert run.outputs == {"first": {"text": "FIRST"}}
    assert calls == ["first"]
    assert execute_workflow_plan(plan, catalog, checkpoint=run.checkpoint).status == "cancelled"
    assert execute_workflow_plan(plan, catalog, checkpoint=run.checkpoint, resume_cancelled=True).status == "completed"
    assert calls == ["first"]


def test_resume_cancellation_does_not_grant_source_dispatch_or_retry_permission():
    token = WorkflowCancellationToken()
    token.cancel()
    catalog, calls, plan = _case()
    stopped = execute_workflow_plan(plan, catalog, cancellation=token)
    unapproved = execute_workflow_plan(plan, catalog, checkpoint=stopped.checkpoint, resume_cancelled=True)
    assert unapproved.status == "approval_required"
    assert unapproved.checkpoint["cancel_requested"] is False
    assert calls == []


def test_failure_during_cancellation_remains_failure_and_needs_both_resume_and_retry():
    token = WorkflowCancellationToken()
    failures = [True]

    def fail_once(_arguments):
        if failures:
            failures.pop()
            token.cancel()
            raise RuntimeError("private source response body")

    catalog, calls, plan = _case(fail_once)
    stopped = execute_workflow_plan(plan, catalog, approved=True, cancellation=token)
    assert stopped.status == "cancelled"
    assert stopped.checkpoint["failure"] == {"step_id": "first", "error_type": "RuntimeError"}
    assert stopped.checkpoint["results"] == {}
    assert stopped.outputs == {}
    assert "private source" not in str(stopped.checkpoint)
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 retry_failed=True).status == "cancelled"
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 resume_cancelled=True).status == "retry_required"
    assert calls == ["first"]
    completed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                     resume_cancelled=True, retry_failed=True)
    assert completed.status == "completed"
    assert calls == ["first", "first", "FIRST"]
    assert completed.checkpoint["attempts"] == {"first": 2, "second": 1}


def test_cancellation_after_persisted_intent_prevents_call_and_does_not_spend_attempt():
    token = WorkflowCancellationToken()
    catalog, calls, plan = _case()
    snapshots = []

    def persist(value):
        snapshots.append(value)
        if value["in_flight"] is not None:
            token.cancel()

    stopped = execute_workflow_plan(plan, catalog, approved=True, cancellation=token, on_checkpoint=persist)
    assert snapshots[0]["status"] == "running"
    assert snapshots[0]["in_flight"] == "first"
    assert stopped.status == "cancelled"
    assert stopped.checkpoint["attempts"] == {}
    assert stopped.checkpoint["in_flight"] is None
    assert calls == []
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 resume_cancelled=True).status == "completed"


def test_cancel_before_retry_dispatch_preserves_the_original_failed_boundary_and_attempt():
    fail = [True]

    def first_failure(_arguments):
        if fail:
            fail.pop()
            raise OSError("fixture source failure")

    catalog, calls, plan = _case(first_failure)
    failed = execute_workflow_plan(plan, catalog, approved=True)
    token = WorkflowCancellationToken()

    def persist(value):
        if value["in_flight"]:
            token.cancel()

    stopped = execute_workflow_plan(plan, catalog, approved=True, checkpoint=failed.checkpoint,
                                   retry_failed=True, cancellation=token, on_checkpoint=persist)
    assert stopped.status == "cancelled"
    assert stopped.checkpoint["attempts"] == {"first": 1}
    assert stopped.checkpoint["failure"] == failed.checkpoint["failure"]
    assert calls == ["first"]
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 resume_cancelled=True).status == "retry_required"
    assert calls == ["first"]


def test_failed_cancellation_persistence_prevents_dispatch_and_preserves_previous_file(tmp_path):
    catalog, calls, plan = _case()
    paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1)
    path = tmp_path / "checkpoint.json"
    save_workflow_checkpoint(path, paused.checkpoint)
    original = path.read_bytes()
    token = WorkflowCancellationToken()

    def unavailable(_value):
        token.cancel()
        raise OSError("fixture storage unavailable")

    with pytest.raises(OSError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint,
                             cancellation=token, on_checkpoint=unavailable)
    assert calls == ["first"]
    assert path.read_bytes() == original


def test_catalogue_drift_from_cancelling_checkpoint_callback_still_prevents_source_call():
    catalog, calls, plan = _case()
    token = WorkflowCancellationToken()

    def mutate(_value):
        token.cancel()
        original = catalog.resolve("fixture.echo")
        catalog.register(WorkflowOperation("fixture.extra", "Extra fixture", original.input_schema,
            original.output_schema, "fixture", implementation="local"), lambda *_a, **_k: None)

    with pytest.raises(WorkflowValidationError, match="catalogue changed"):
        execute_workflow_plan(plan, catalog, approved=True, cancellation=token, on_checkpoint=mutate)
    assert calls == []


def test_cancelled_unknown_completion_still_requires_explicit_retry_after_resume():
    catalog, calls, plan = _case()
    captured = []

    def unavailable(value):
        captured.append(value)
        raise OSError("fixture persistence unavailable")

    with pytest.raises(OSError):
        execute_workflow_plan(plan, catalog, approved=True, on_checkpoint=unavailable)
    token = WorkflowCancellationToken()
    token.cancel()
    stopped = execute_workflow_plan(plan, catalog, approved=True, checkpoint=captured[0], cancellation=token)
    assert stopped.status == "cancelled"
    assert stopped.checkpoint["in_flight"] == "first"
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 resume_cancelled=True).status == "retry_required"
    assert calls == []


def test_cancel_resume_cannot_reset_fixed_operation_call_caps():
    token = WorkflowCancellationToken()

    def fail(_arguments):
        token.cancel()
        raise OSError("fixture unavailable")

    requirements = compile_workflow_requirements("Read fixture text", max_calls_by_operation={"fixture.echo": 1})
    catalog, calls, plan = _case(fail, one_step=True, requirements=requirements)
    stopped = execute_workflow_plan(plan, catalog, approved=True, cancellation=token)
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                   resume_cancelled=True, retry_failed=True)
    assert resumed.status == "blocked"
    assert resumed.checkpoint["failure"]["error_type"] == "WorkflowRequirementError"
    assert resumed.checkpoint["attempts"] == {"first": 1}
    assert calls == ["first"]


def test_cancel_resume_cannot_reset_the_per_step_attempt_limit():
    def fail(_arguments):
        raise OSError("fixture unavailable")

    catalog, calls, plan = _case(fail, one_step=True)
    checkpoint = None
    for _ in range(MAX_WORKFLOW_ATTEMPTS):
        failed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint, retry_failed=True)
        token = WorkflowCancellationToken()
        token.cancel()
        stopped = execute_workflow_plan(plan, catalog, approved=True, checkpoint=failed.checkpoint, cancellation=token)
        checkpoint = execute_workflow_plan(plan, catalog, checkpoint=stopped.checkpoint,
                                           resume_cancelled=True).checkpoint
    limited = execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint, retry_failed=True)
    assert limited.status == "attempt_limit"
    assert len(calls) == MAX_WORKFLOW_ATTEMPTS
    token = WorkflowCancellationToken()
    token.cancel()
    stopped = execute_workflow_plan(plan, catalog, approved=True, checkpoint=limited.checkpoint, cancellation=token)
    assert stopped.status == "cancelled"
    unapproved = execute_workflow_plan(plan, catalog, checkpoint=stopped.checkpoint, resume_cancelled=True)
    assert unapproved.status == "attempt_limit"
    assert execute_workflow_plan(plan, catalog, checkpoint=unapproved.checkpoint).status == "attempt_limit"
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=stopped.checkpoint,
                                 resume_cancelled=True, retry_failed=True).status == "attempt_limit"
    assert len(calls) == MAX_WORKFLOW_ATTEMPTS


@pytest.mark.parametrize("mutate", [
    lambda value: value.update(cancel_requested=1),
    lambda value: value.update(cancel_requested=True),
    lambda value: value.update(status="cancelled"),
    lambda value: value.pop("cancel_requested"),
    lambda value: value.update(schema=WORKFLOW_CHECKPOINT_SCHEMA_V01),
])
def test_rehashed_checkpoint_cannot_encode_inconsistent_cancellation(mutate):
    catalog, calls, plan = _case()
    checkpoint = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    mutate(checkpoint)
    with pytest.raises(WorkflowValidationError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=_restamp(checkpoint))
    assert calls == ["first"]


@pytest.mark.parametrize("state", ["paused", "failed", "running"])
def test_legacy_checkpoint_upgrades_without_losing_results_failure_or_unknown_boundary(state):
    catalog, calls, plan = _case()
    if state == "paused":
        value = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    elif state == "failed":
        catalog, calls, plan = _case(lambda _: (_ for _ in ()).throw(OSError("fixture")))
        value = execute_workflow_plan(plan, catalog, approved=True).checkpoint
    else:
        values = []
        def unavailable(checkpoint):
            values.append(checkpoint)
            raise OSError("fixture")
        with pytest.raises(OSError):
            execute_workflow_plan(plan, catalog, approved=True, on_checkpoint=unavailable)
        value = values[0]
    legacy = deepcopy(value)
    legacy.pop("cancel_requested")
    legacy["schema"] = WORKFLOW_CHECKPOINT_SCHEMA_V01
    token = WorkflowCancellationToken()
    token.cancel()
    held = execute_workflow_plan(plan, catalog, approved=True, checkpoint=_restamp(legacy), cancellation=token)
    assert held.status == "cancelled"
    assert held.checkpoint["schema"] == WORKFLOW_CHECKPOINT_SCHEMA
    for key in ("results", "attempts", "failure", "in_flight"):
        assert held.checkpoint[key] == value[key]


@pytest.mark.parametrize("kwargs", [{"cancellation": True}, {"cancellation": Event()}, {"resume_cancelled": 1}])
def test_invalid_cancellation_controls_fail_before_dispatch(kwargs):
    catalog, calls, plan = _case()
    with pytest.raises(WorkflowValidationError):
        execute_workflow_plan(plan, catalog, approved=True, **kwargs)
    assert calls == []
