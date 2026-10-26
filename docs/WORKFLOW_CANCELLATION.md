# Cooperative workflow cancellation

This repository-defined application contract extends the reviewed read-only DAG in
`python/prism_sdk/workflow_builder.py`. It follows the existing worker drain rule in
`_async_callback_bridge.run_in_thread_and_drain`: a stop request prevents further dispatch at
checked boundaries; an admitted synchronous call must settle. It introduces no additional
execution, effect, or authorization ledger and makes no blueprint implementation claim.

## SDK control

```python
from prism_sdk.workflow_builder import (
    WorkflowCancellationToken,
    execute_workflow_plan,
    load_workflow_checkpoint,
    save_workflow_checkpoint,
)

control = WorkflowCancellationToken()

# A caller-owned thread or checkpoint callback can call control.cancel().
# This request is one-way and carries no provider, source, or retry permission.
run = execute_workflow_plan(
    plan,
    catalog,
    approved=True,
    cancellation=control,
    on_checkpoint=lambda value: save_workflow_checkpoint(checkpoint_path, value),
)

if run.status == "cancelled":
    # The caller explicitly decides to continue, using a fresh token if needed.
    continued = execute_workflow_plan(
        plan,
        catalog,
        checkpoint=load_workflow_checkpoint(checkpoint_path),
        cancellation=WorkflowCancellationToken(),
        resume_cancelled=True,
        approved=True,
        # retry_failed=True is separately required for a failed/unknown call.
    )
```

`WorkflowCancellationToken.cancel()` is thread-safe and returns without waiting for an active
call. Its `cancellation_requested` property remains true. The executor checks the request before
work, between steps, after persisting a dispatch intent, and after retaining a call's outcome.
A persistence callback can request cancellation; the executor checks again before calling the
source. A request concurrent with dispatch admission can leave that already-admitted call to
finish. There is no interruption or rollback promise.

The executor retains completed public values and receipts. A source exception during a stop
request remains a recorded failure, with its sanitized error class and no invented result.
An interrupted call retained as `in_flight` remains an unknown-completion boundary. Cancelling
after the final call can therefore return `cancelled` with the actual final output available;
the graph's observations and the caller's stop request are separate facts.

## Durable checkpoints and independent grants

New checkpoints use `aurora-executable-workflow-checkpoint/0.2` and include the mandatory literal
boolean `cancel_requested`. Its true state is represented by `status: "cancelled"`. The executor
accepts valid 0.1 checkpoints and upgrades them with `cancel_requested: false`; 0.1 cannot encode
a cancellation. Retained argument/output schemas, dependency closure, catalogue binding, fixed
requirements, and existing attempt/call caps are revalidated during reload.

A cancelled checkpoint stays cancelled until the caller passes `resume_cancelled=True`. An
already-requested token continues to hold it even with that grant; use no token or a fresh token
to continue. Resume does not confer `approved=True`, provider approval, or `retry_failed=True`.
For example, resuming a cancelled failure without retry permission returns `retry_required`
and makes no call. Successfully completed steps are never repeated by cancellation/resume.

`save_workflow_checkpoint` atomically replaces the selected public-value checkpoint. The
executor waits for the cancellation checkpoint callback to succeed before returning a cancelled
run. A callback/persistence exception propagates: durability is not claimed when persistence
fails. Failure of the pre-dispatch callback prevents the source call. If a callback cancels
after a successful intent write but before dispatch, the executor restores the previous attempt
and failure/unknown boundary and writes the cancellation; that uncalled attempt consumes no
call budget. A crash between these writes conservatively leaves the existing unknown boundary.

Checkpoint digests detect content mismatch; they are not signatures or an authorization server.
The application owns the checkpoint location and caller grants. Public-value checkpoint exports
retain source values by explicit choice; the metadata projection continues to omit those values.

## CLI boundary

CLI workflows expose `--cancel-file` as an operator-selected sentinel. Its presence is checked
at startup and checkpoint boundaries, including immediately before source dispatch; it does
not run a watcher thread or abort an outstanding response. The checkpoint records an observed
request. Remove the sentinel and use `workflow-execute --resume --resume-cancelled` with the
independent source/provider/retry grants needed for the remaining work. A cancelled CLI outcome
uses exit code 130. Recreating a fresh plan is a separate run, rather than resuming this state.

## Explicit limits and verification

This control applies to the current serial read-only executor. It does not implement process
termination, asynchronous transport cancellation, concurrent step scheduling, conditional
branches, effectful writes, distributed leases, exactly-once execution, rollback, clinical
approval, or scientific validation. Existing effectful workflows still require their separate
`AutonomousEffectBoundary` reconciliation and `AutonomousAuthorizationContext` contracts.

`python/tests/test_workflow_cancellation.py` verifies cancellation before dispatch, callback
requests between steps and after intent persistence, blocking-call drain, preservation of source
failure and receipts, reload holds and independent resume/retry/source grants, legacy upgrades,
persistence failure, catalogue drift, and immutable call/attempt limits. Fixtures exercise actual
in-process executor calls; they do not claim remote cancellation was tested.
