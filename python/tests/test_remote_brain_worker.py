from __future__ import annotations

import asyncio
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AsyncBrainControlClient,
    AsyncDurableBrainControlPlaneAdapter,
    AsyncRemoteBrainJobWorker,
    BrainControlClient,
    BrainJobStore,
    DurableBrainControlPlaneAdapter,
    RemoteBrainJobWorker,
    RemoteBrainWorkerError,
    ProvisionedRemoteBrainCredentialScope,
    autonomous_remote_brain_job_spec_digest,
    autonomous_remote_brain_plan_digest,
    autonomous_remote_brain_route_digest,
)


class _Result:
    def __init__(self, status: str = "completed") -> None:
        self.status = status
        self.outcome_digest = "a" * 64
        self.plan_digest = "b" * 64


class _Brain:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def _run(self, name: str, **kwargs: object) -> _Result:
        self.calls.append((name, dict(kwargs)))
        return _Result()

    def run_autonomous(self, **kwargs: object) -> _Result:
        return self._run("run_autonomous", **kwargs)

    def run_workflow(self, **kwargs: object) -> _Result:
        return self._run("run_workflow", **kwargs)

    def run_workflow_learning(self, **kwargs: object) -> _Result:
        return self._run("run_workflow_learning", **kwargs)

    def run_workflow_cycle(self, **kwargs: object) -> _Result:
        return self._run("run_workflow_cycle", **kwargs)

    def run_workflow_trajectory_learning(self, **kwargs: object) -> _Result:
        return self._run("run_workflow_trajectory_learning", **kwargs)

    def run_cross_domain(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain", **kwargs)

    def run_cross_domain_learning(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain_learning", **kwargs)

    def run_cross_domain_trajectory_learning(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain_trajectory_learning", **kwargs)

    def run_cross_domain_replan_learning(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain_replan_learning", **kwargs)


class _ReconcileBrain(_Brain):
    def run_autonomous(self, **kwargs: object) -> _Result:
        self.calls.append(("run_autonomous", dict(kwargs)))
        return _Result("reconciliation_required")


class _BlockingAsyncBrain(_Brain):
    def __init__(self) -> None:
        super().__init__()
        self.started = asyncio.Event()

    async def run_autonomous(self, **kwargs: object) -> _Result:
        self.calls.append(("run_autonomous", dict(kwargs)))
        self.started.set()
        await asyncio.Event().wait()
        return _Result()


class _ProvisionedSession:
    def __init__(self) -> None:
        self.closed = False
        self.handle = object()

    def handles(self) -> dict[str, object]:
        if self.closed:
            raise RuntimeError("session already closed")
        return {"groq": self.handle}

    def close(self) -> None:
        self.closed = True


class _ProvisioningBrain(_Brain):
    def __init__(self) -> None:
        super().__init__()
        self.sessions: list[_ProvisionedSession] = []
        self.provisioning_calls: list[dict[str, object]] = []

    def start_provisioned_credential_session(self, **kwargs: object) -> tuple[_ProvisionedSession, dict[str, object]]:
        self.provisioning_calls.append(dict(kwargs))
        session = _ProvisionedSession()
        self.sessions.append(session)
        return session, {"ready": True}


def _control(tmp_path, seen: list[tuple[str, dict[str, object]]]) -> tuple[BrainControlClient, BrainJobStore]:
    store = BrainJobStore(tmp_path / "remote-brain.sqlite3")

    def call_tool(name: str, arguments: dict[str, object]) -> dict[str, object]:
        seen.append((name, dict(arguments)))
        return adapter.call_tool(name, arguments)

    adapter = DurableBrainControlPlaneAdapter(store, authorizer=lambda _operation, _metadata: True)
    return BrainControlClient.from_durable(type("RecordingAdapter", (), {"call_tool": staticmethod(call_tool)})()), store


async def _async_control(tmp_path, seen: list[tuple[str, dict[str, object]]]) -> tuple[AsyncBrainControlClient, BrainJobStore]:
    store = BrainJobStore(tmp_path / "async-remote-brain.sqlite3")
    adapter = AsyncDurableBrainControlPlaneAdapter(
        DurableBrainControlPlaneAdapter(store, authorizer=lambda _operation, _metadata: True)
    )

    class RecordingAdapter:
        async def call_tool(self, name: str, arguments: dict[str, object]) -> dict[str, object]:
            seen.append((name, dict(arguments)))
            return await adapter.call_tool(name, arguments)

    return AsyncBrainControlClient.from_durable(RecordingAdapter()), store


def _policy(letter: str) -> str:
    return letter * 64


def test_remote_worker_provisions_opaque_handles_only_after_approval_and_closes_them(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _ProvisioningBrain()
    scope = ProvisionedRemoteBrainCredentialScope(
        brain,
        providers=("groq",),
        ttl_seconds=60,
        environ={},
    )
    request = {"task": "private provisioned task", "domain": "coding"}
    worker = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="provisioned-worker",
        credential_scope=scope,
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"]},
        },
    )
    submitted = worker.submit(
        idempotency_key="provisioned-scope",
        request=request,
        mode="autonomous",
        domain="coding",
        capability="bounded",
        risk_class="review",
    )
    assert submitted.job is not None
    waiting = worker.run_once(submitted.job["job_id"])
    assert waiting is not None and waiting.status == "waiting_approval"
    assert brain.provisioning_calls == []
    assert brain.sessions == []

    worker.approval(submitted.job["job_id"], "approve", authorization_digest="d" * 64)
    completed = worker.run_once(submitted.job["job_id"])
    assert completed is not None and completed.status == "succeeded"
    assert len(brain.provisioning_calls) == 1
    assert brain.provisioning_calls[0]["require_ready"] is True
    assert brain.calls[-1][1]["credentials"] == {"groq": brain.sessions[0].handle}
    assert brain.sessions[0].closed is True
    assert all("credentials" not in arguments for _name, arguments in seen)
    assert "private provisioned task" not in json.dumps([record.to_dict() for record in store.inventory(limit=64)])
    store.close()


def test_async_remote_worker_provisions_and_closes_scope_in_worker_lifecycle(tmp_path):
    asyncio.run(_run_async_provisioned_scope(tmp_path))


async def _run_async_provisioned_scope(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = await _async_control(tmp_path, seen)
    brain = _ProvisioningBrain()
    scope = ProvisionedRemoteBrainCredentialScope(brain, providers=("groq",))
    request = {"task": "private async provisioned task", "domain": "research"}
    worker = AsyncRemoteBrainJobWorker(
        brain,
        control,
        worker_id="async-provisioned-worker",
        credential_scope=scope,
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"]},
        },
    )
    submitted = await worker.submit(
        idempotency_key="async-provisioned-scope",
        request=request,
        mode="autonomous",
        domain="research",
        capability="bounded",
        risk_class="review",
    )
    assert submitted.job is not None
    waiting = await worker.run_once(submitted.job["job_id"])
    assert waiting is not None and waiting.status == "waiting_approval"
    assert brain.sessions == []
    await worker.approval(submitted.job["job_id"], "approve", authorization_digest="e" * 64)
    completed = await worker.run_once(submitted.job["job_id"])
    assert completed is not None and completed.status == "succeeded"
    assert len(brain.sessions) == 1 and brain.sessions[0].closed is True
    assert brain.calls[-1][1]["credentials"] == {"groq": brain.sessions[0].handle}
    assert all("credentials" not in arguments for _name, arguments in seen)
    store.close()


def test_remote_worker_approval_gates_all_modes_and_all_domains_without_remote_private_values(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _Brain()
    modes = (
        "autonomous",
        "workflow",
        "workflow_learning",
        "workflow_cycle",
        "workflow_trajectory_learning",
        "cross_domain",
        "cross_domain_learning",
        "cross_domain_trajectory_learning",
        "cross_domain_replan",
    )
    jobs: dict[str, tuple[str, dict[str, object], str]] = {}
    for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES):
        mode = modes[index % len(modes)]
        request = {"task": f"private {domain} task", "domain": domain}
        policy = _policy("abcdef"[index % 6])
        submission = RemoteBrainJobWorker(
            brain,
            control,
            worker_id=f"submitter-{index}",
            resolver=lambda _context: {},
        ).submit(
            idempotency_key=f"remote-{mode}-{index}",
            request=request,
            mode=mode,
            domain="cross_domain" if mode.startswith("cross_domain") else domain,
            capability="bounded_capability",
            risk_class="review",
            policy_digest=policy,
        )
        assert submission.status == "submitted"
        assert submission.job is not None
        jobs[submission.job["job_id"]] = (mode, request, policy)

    worker = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="remote-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "policy_digest": jobs[context["job"]["job_id"]][2],
            "mode": jobs[context["job"]["job_id"]][0],
            "request": jobs[context["job"]["job_id"]][1],
            "kwargs": {"task": jobs[context["job"]["job_id"]][1]["task"], "domain": jobs[context["job"]["job_id"]][1]["domain"]},
        },
    )

    for job_id, (mode, _request, _policy_value) in jobs.items():
        calls_before = len(brain.calls)
        waiting = worker.run_once(job_id)
        assert waiting is not None
        assert waiting.status == "waiting_approval"
        assert len(brain.calls) == calls_before
        worker.approval(job_id, "approve", authorization_digest="c" * 64)
        completed = worker.run_once(job_id)
        assert completed is not None
        assert completed.status == "succeeded", mode
        assert brain.calls[-1][0] == worker._RUNNERS[mode]

    assert len(brain.calls) == len(jobs)
    assert all("task" not in arguments and "prompt" not in arguments for _name, arguments in seen)
    serialized = json.dumps([record.to_dict() for record in store.inventory(limit=64)], sort_keys=True)
    assert "private " not in serialized
    store.close()


def test_remote_worker_rejects_spec_drift_before_dispatch_and_requeues_typed_preflight_failure(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _Brain()
    request = {"task": "private original task", "domain": "coding"}
    policy = _policy("d")
    drift = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="drift-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "policy_digest": policy,
            "mode": "autonomous",
            "request": {**request, "task": "tampered task"},
            "kwargs": {"task": "tampered task", "domain": "coding"},
        },
    )
    submitted = drift.submit(idempotency_key="drift", request=request, mode="autonomous", domain="coding", capability="bounded", risk_class="review", policy_digest=policy)
    failed = drift.run_once(submitted.job["job_id"])
    assert failed is not None
    assert failed.status == "failed"
    assert brain.calls == []
    assert "tampered task" not in json.dumps([record.to_dict() for record in store.inventory(limit=64)])

    retry = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="retry-worker",
        resolver=lambda _context: (_ for _ in ()).throw(RemoteBrainWorkerError("temporary resolver outage", code="transport", retryable=True)),
    )
    retry_submission = retry.submit(idempotency_key="retry", request=request, mode="autonomous", domain="coding", capability="bounded", risk_class="review", policy_digest=policy)
    scheduled = retry.run_once(retry_submission.job["job_id"])
    assert scheduled is not None
    assert scheduled.status == "retry_scheduled"
    assert store.get(retry_submission.job["job_id"]).state == "queued"
    store.close()


def test_remote_worker_binds_reviewed_plan_and_route_identity_without_remote_private_values(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _Brain()
    request = {"task": "private reviewed plan task", "domain": "engineering"}
    blueprint = {"spec": {"task": request["task"]}, "plan": {"steps": ["review", "execute"]}}
    route = {"status": "ready", "selected_domains": ["engineering"]}
    plan_digest = autonomous_remote_brain_plan_digest(blueprint)
    route_digest = autonomous_remote_brain_route_digest(route)
    resolved = lambda _context: {
        "spec_digest": submission.spec_digest,
        "mode": "autonomous",
        "request": request,
        "kwargs": {"task": request["task"], "domain": request["domain"], "blueprint": blueprint, "route": route},
        "plan_digest": plan_digest,
        "route_digest": route_digest,
    }
    worker = RemoteBrainJobWorker(brain, control, worker_id="reviewed-plan-worker", resolver=resolved)
    submission = worker.submit(
        idempotency_key="reviewed-plan",
        request=request,
        mode="autonomous",
        domain="engineering",
        capability="bounded",
        risk_class="review",
        plan_digest=plan_digest,
        route_digest=route_digest,
    )
    assert submission.plan_digest == plan_digest
    assert submission.route_digest == route_digest
    assert submission.to_dict()["private_spec"].startswith("caller_owned")
    waiting = worker.run_once(submission.job["job_id"])
    assert waiting is not None and waiting.status == "waiting_approval"
    worker.approval(submission.job["job_id"], "approve", authorization_digest="a" * 64)
    completed = worker.run_once(submission.job["job_id"])
    assert completed is not None and completed.status == "succeeded"
    assert all("private reviewed plan task" not in json.dumps(arguments) for _name, arguments in seen)
    assert "private reviewed plan task" not in json.dumps([record.to_dict() for record in store.inventory(limit=64)])

    tampered_blueprint = {"spec": {"task": request["task"]}, "plan": {"steps": ["tampered"]}}
    tampered_worker = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="tampered-plan-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"], "blueprint": tampered_blueprint, "route": route},
            "plan_digest": plan_digest,
            "route_digest": route_digest,
        },
    )
    tampered = tampered_worker.submit(
        idempotency_key="tampered-reviewed-plan",
        request=request,
        mode="autonomous",
        domain="engineering",
        capability="bounded",
        risk_class="review",
        plan_digest=plan_digest,
        route_digest=route_digest,
    )
    rejected = tampered_worker.run_once(tampered.job["job_id"])
    assert rejected is not None and rejected.status == "failed"
    assert len(brain.calls) == 1
    store.close()


def test_async_remote_worker_preserves_all_domains_modes_and_metadata_boundary(tmp_path):
    asyncio.run(_run_async_remote_worker(tmp_path))


def test_async_remote_worker_binds_reviewed_identities(tmp_path):
    asyncio.run(_run_async_reviewed_identities(tmp_path))


async def _run_async_reviewed_identities(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = await _async_control(tmp_path, seen)
    brain = _Brain()
    request = {"task": "private async reviewed task", "domain": "research"}
    blueprint = {"spec": {"task": request["task"]}, "plan": {"steps": ["research"]}}
    route = {"status": "ready", "selected_domains": ["research"]}
    plan_digest = autonomous_remote_brain_plan_digest(blueprint)
    route_digest = autonomous_remote_brain_route_digest(route)
    worker = AsyncRemoteBrainJobWorker(
        brain,
        control,
        worker_id="async-reviewed-worker",
        resolver=lambda _context: {
            "spec_digest": submission.spec_digest,
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"], "blueprint": blueprint, "route": route},
            "plan_digest": plan_digest,
            "route_digest": route_digest,
        },
    )
    submission = await worker.submit(
        idempotency_key="async-reviewed-identities",
        request=request,
        mode="autonomous",
        domain="research",
        capability="bounded",
        risk_class="review",
        plan_digest=plan_digest,
        route_digest=route_digest,
    )
    assert submission.plan_digest == plan_digest
    assert submission.route_digest == route_digest
    waiting = await worker.run_once(submission.job["job_id"])
    assert waiting is not None and waiting.status == "waiting_approval"
    await worker.approval(submission.job["job_id"], "approve", authorization_digest="b" * 64)
    completed = await worker.run_once(submission.job["job_id"])
    assert completed is not None and completed.status == "succeeded"
    assert all("private async reviewed task" not in json.dumps(arguments) for _name, arguments in seen)
    store.close()


async def _run_async_remote_worker(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = await _async_control(tmp_path, seen)
    brain = _Brain()
    modes = (
        "autonomous",
        "workflow",
        "workflow_learning",
        "workflow_cycle",
        "workflow_trajectory_learning",
        "cross_domain",
        "cross_domain_learning",
        "cross_domain_trajectory_learning",
        "cross_domain_replan",
    )
    jobs: dict[str, tuple[str, dict[str, object], str]] = {}
    for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES):
        mode = modes[index % len(modes)]
        request = {"task": f"private async {domain} task", "domain": domain}
        policy = _policy("abcdef"[index % 6])
        submission = await AsyncRemoteBrainJobWorker(
            brain,
            control,
            worker_id=f"async-submitter-{index}",
            resolver=lambda _context: {},
        ).submit(
            idempotency_key=f"async-remote-{mode}-{index}",
            request=request,
            mode=mode,
            domain="cross_domain" if mode.startswith("cross_domain") else domain,
            capability="bounded_capability",
            risk_class="review",
            policy_digest=policy,
        )
        assert submission.job is not None
        jobs[submission.job["job_id"]] = (mode, request, policy)

    worker = AsyncRemoteBrainJobWorker(
        brain,
        control,
        worker_id="async-remote-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "policy_digest": jobs[context["job"]["job_id"]][2],
            "mode": jobs[context["job"]["job_id"]][0],
            "request": jobs[context["job"]["job_id"]][1],
            "kwargs": {
                "task": jobs[context["job"]["job_id"]][1]["task"],
                "domain": jobs[context["job"]["job_id"]][1]["domain"],
            },
        },
    )
    for job_id, (mode, _request, _policy_value) in jobs.items():
        waiting = await worker.run_once(job_id)
        assert waiting is not None
        assert waiting.status == "waiting_approval"
        await worker.approval(job_id, "approve", authorization_digest="c" * 64)
        completed = await worker.run_once(job_id)
        assert completed is not None
        assert completed.status == "succeeded", mode
        assert brain.calls[-1][0] == worker._RUNNERS[mode]

    empty = await worker.run(limit=1)
    assert empty.status == "empty"
    assert len(brain.calls) == len(jobs)
    assert all("task" not in arguments and "prompt" not in arguments for _name, arguments in seen)
    serialized = json.dumps([record.to_dict() for record in store.inventory(limit=64)], sort_keys=True)
    assert "private async" not in serialized
    store.close()


def test_async_remote_worker_quarantines_cancelled_active_dispatch(tmp_path):
    asyncio.run(_run_async_cancellation(tmp_path))


async def _run_async_cancellation(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = await _async_control(tmp_path, seen)
    brain = _BlockingAsyncBrain()
    request = {"task": "private cancellable task", "domain": "operations"}
    policy = _policy("f")
    worker = AsyncRemoteBrainJobWorker(
        brain,
        control,
        worker_id="async-cancellation-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "policy_digest": policy,
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"]},
        },
    )
    submitted = await worker.submit(
        idempotency_key="async-cancellation",
        request=request,
        mode="autonomous",
        domain="operations",
        capability="bounded",
        risk_class="review",
        policy_digest=policy,
    )
    assert submitted.job is not None
    await worker.run_once(submitted.job["job_id"])
    await worker.approval(submitted.job["job_id"], "approve", authorization_digest="1" * 64)
    active = asyncio.create_task(worker.run_once(submitted.job["job_id"]))
    await brain.started.wait()
    active.cancel()
    with pytest.raises(asyncio.CancelledError):
        await active
    assert store.get(submitted.job["job_id"]).state == "reconciliation_required"
    assert len(brain.calls) == 1
    store.close()


def test_remote_worker_quarantines_runner_reconciliation_and_accepts_explicit_evidence(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _ReconcileBrain()
    request = {"task": "private uncertain task", "domain": "engineering"}
    policy = _policy("e")
    worker = RemoteBrainJobWorker(
        brain,
        control,
        worker_id="reconcile-worker",
        resolver=lambda context: {
            "spec_digest": context["job"]["spec_digest"],
            "policy_digest": policy,
            "mode": "autonomous",
            "request": request,
            "kwargs": {"task": request["task"], "domain": request["domain"]},
        },
    )
    submitted = worker.submit(
        idempotency_key="reconcile",
        request=request,
        mode="autonomous",
        domain="engineering",
        capability="bounded",
        risk_class="review",
        policy_digest=policy,
    )
    assert submitted.job is not None
    assert worker.run_once(submitted.job["job_id"]).status == "waiting_approval"
    worker.approval(submitted.job["job_id"], "approve", authorization_digest="f" * 64)
    quarantined = worker.run_once(submitted.job["job_id"])
    assert quarantined is not None
    assert quarantined.status == "reconciliation_required"
    assert store.get(submitted.job["job_id"]).state == "reconciliation_required"
    assert len(brain.calls) == 1
    worker.reconcile(
        submitted.job["job_id"],
        outcome="succeeded",
        evidence_digest="1" * 64,
        evidence_kind="caller_receipt",
        operator="operator-1",
    )
    assert store.get(submitted.job["job_id"]).state == "succeeded"
    store.close()


def test_remote_spec_digest_is_stable_and_rejects_non_json_private_policy():
    request = {"task": "bounded task", "domain": "science", "context": ["bounded"]}
    first = autonomous_remote_brain_job_spec_digest(request=request, mode="autonomous", policy_digest="e" * 64)
    second = autonomous_remote_brain_job_spec_digest(request=dict(request), mode="autonomous", policy_digest="e" * 64)
    assert first == second
    with_identities = autonomous_remote_brain_job_spec_digest(
        request=request,
        mode="autonomous",
        policy_digest="e" * 64,
        plan_digest="a" * 64,
        route_digest="b" * 64,
    )
    assert with_identities != first
    assert with_identities == autonomous_remote_brain_job_spec_digest(
        request=dict(request),
        mode="autonomous",
        policy_digest="e" * 64,
        plan_digest="a" * 64,
        route_digest="b" * 64,
    )
    assert with_identities != autonomous_remote_brain_job_spec_digest(
        request=request,
        mode="autonomous",
        policy_digest="e" * 64,
        plan_digest="c" * 64,
        route_digest="b" * 64,
    )
    with pytest.raises(RemoteBrainWorkerError):
        autonomous_remote_brain_job_spec_digest(request={"task": object()}, mode="autonomous")
