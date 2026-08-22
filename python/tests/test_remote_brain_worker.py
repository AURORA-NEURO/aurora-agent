from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    BrainControlClient,
    BrainJobStore,
    DurableBrainControlPlaneAdapter,
    RemoteBrainJobWorker,
    RemoteBrainWorkerError,
    autonomous_remote_brain_job_spec_digest,
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

    def run_cross_domain(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain", **kwargs)

    def run_cross_domain_learning(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain_learning", **kwargs)

    def run_cross_domain_replan_learning(self, **kwargs: object) -> _Result:
        return self._run("run_cross_domain_replan_learning", **kwargs)


class _ReconcileBrain(_Brain):
    def run_autonomous(self, **kwargs: object) -> _Result:
        self.calls.append(("run_autonomous", dict(kwargs)))
        return _Result("reconciliation_required")


def _control(tmp_path, seen: list[tuple[str, dict[str, object]]]) -> tuple[BrainControlClient, BrainJobStore]:
    store = BrainJobStore(tmp_path / "remote-brain.sqlite3")

    def call_tool(name: str, arguments: dict[str, object]) -> dict[str, object]:
        seen.append((name, dict(arguments)))
        return adapter.call_tool(name, arguments)

    adapter = DurableBrainControlPlaneAdapter(store, authorizer=lambda _operation, _metadata: True)
    return BrainControlClient.from_durable(type("RecordingAdapter", (), {"call_tool": staticmethod(call_tool)})()), store


def _policy(letter: str) -> str:
    return letter * 64


def test_remote_worker_approval_gates_all_modes_and_all_domains_without_remote_private_values(tmp_path):
    seen: list[tuple[str, dict[str, object]]] = []
    control, store = _control(tmp_path, seen)
    brain = _Brain()
    modes = (
        "autonomous",
        "workflow",
        "workflow_learning",
        "workflow_cycle",
        "cross_domain",
        "cross_domain_learning",
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
    with pytest.raises(RemoteBrainWorkerError):
        autonomous_remote_brain_job_spec_digest(request={"task": object()}, mode="autonomous")
