from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AutonomousBrain,
    BrainApprovalRouter,
    BrainControlPlane,
    BrainJobRunResult,
    BrainJobStore,
    BrainModelHealthStore,
    BrainModelObservation,
    BrainReplayCase,
    BrainReplayEngine,
    DomainEvaluatorRegistry,
    BrainWorker,
    CredentialStore,
    LLMRuntime,
    openai_provider,
    ProviderError,
    ProviderRequest,
)
from prism_sdk.brain import BrainRunError


def _digest(value: object) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _packet(key: str = "control-plane-job") -> dict[str, object]:
    return {
        "idempotency_key": key,
        "spec_digest": "a" * 64,
        "domain": "engineering",
        "capability": "release_audit",
        "risk_class": "release_review",
        "max_attempts": 2,
    }


def _evidence(domain: str) -> dict[str, object]:
    signals = {
        "engineering": {"schema_valid": True, "tests_passed": True, "evidence_complete": True},
        "research": {"evidence_traceable": True, "uncertainty_reported": True, "claim_scope_respected": True},
        "operations": {"safety_gate_passed": True, "approval_complete": True, "rollback_plan_present": True},
        "data": {"schema_valid": True, "lineage_complete": True, "quality_gate_passed": True},
        "biomedical": {"boundary_compliant": True, "provenance_complete": True, "human_review_ready": True},
    }[domain]
    return {
        "domain": domain,
        "capability": "bounded_review",
        "risk_class": "review",
        "signals": signals,
        "references": ["b" * 64],
        "limitations": ["caller supplied normalized evidence"],
    }


def _case(domain: str) -> dict[str, object]:
    evidence = _evidence(domain)
    identities = {
        "engineering": ("domain-engineering-quality", "1"),
        "research": ("domain-research-quality", "1"),
        "operations": ("domain-operations-quality", "1"),
        "data": ("domain-data-quality", "1"),
        "biomedical": ("domain-biomedical-boundary", "1"),
    }
    evaluator_id, evaluator_version = identities[domain]
    return {
        "run_id": f"replay-{domain}",
        "domain": domain,
        "capability": "bounded_review",
        "risk_class": "review",
        "evaluator_id": evaluator_id,
        "evaluator_version": evaluator_version,
        "evidence": evidence,
        "evidence_digest": _digest(evidence),
    }


class _StubBrain:
    def run_resumable_learning_job(self, store: BrainJobStore, **kwargs: object) -> BrainJobRunResult:
        job_id = kwargs["job_id"]
        worker_id = kwargs["worker_id"]
        completed = store.complete(job_id, worker_id, result_metadata={"stub": True})
        return BrainJobRunResult(status=completed.state, job=completed.to_dict(), cycle=None)


class _RuntimeFailureBrain:
    def __init__(self) -> None:
        credentials = CredentialStore()
        self.runtime = LLMRuntime(credentials)
        self.runtime.register_provider(
            openai_provider(
                base_url="http://127.0.0.1:1",
                allow_insecure_http=True,
                timeout_seconds=0.2,
                max_attempts=1,
            )
        )
        self.handle = credentials.register("openai", "worker-secret")

    def run_resumable_learning_job(self, store: BrainJobStore, **kwargs: object) -> BrainJobRunResult:
        try:
            self.runtime.invoke(
                "openai",
                ProviderRequest(model="failure-model", messages=({"role": "user", "content": "private"},)),
                credential=self.handle,
            )
        except ProviderError:
            job = store.fail(kwargs["job_id"], kwargs["worker_id"], reason="provider transport failed")
            return BrainJobRunResult(status=job.state, job=job.to_dict(), cycle=None, error_class="ProviderError")
        raise AssertionError("the unavailable provider unexpectedly succeeded")


class _ExplodingBrain:
    def run_resumable_learning_job(self, *_args: object, **_kwargs: object) -> BrainJobRunResult:
        raise RuntimeError("unexpected worker exception")


def test_control_plane_exposes_cursor_events_and_hash_head(tmp_path):
    with BrainJobStore(tmp_path / "jobs.sqlite3") as store:
        plane = BrainControlPlane(store)
        record, receipt = plane.submit(_packet())
        assert receipt["idempotent"] is False
        first = plane.events(limit=1)
        assert first.events[0].event_type == "job_submitted"
        assert first.events[0].event_digest == first.head_digest
        assert first.next_after == first.events[0].sequence

        claimed = store.claim(record.job_id, "worker-a")
        store.checkpoint(
            claimed.job_id,
            "worker-a",
            phase="preflight",
            checkpoint={"route_digest": "c" * 64},
            side_effect_boundary="preflight",
        )
        store.complete(claimed.job_id, "worker-a", result_metadata={"status": "ok"})
        second = plane.events(after_sequence=first.next_after, limit=32)
        assert [event.event_type for event in second.events] == [
            "job_claimed",
            "job_checkpointed",
            "job_completed",
        ]
        assert second.head_digest == store.head_digest()
        assert store.verify_integrity()["ok"] is True


def test_approval_router_persists_approval_and_denial_without_raw_scope(tmp_path):
    with BrainJobStore(tmp_path / "jobs.sqlite3") as store:
        router = BrainApprovalRouter(store)
        job, _ = store.submit(_packet("approve-me"))
        store.claim(job.job_id, "worker-a")
        request = router.request(
            job.job_id,
            "worker-a",
            approval_scope="dispatch release evidence mission",
            request_digest="d" * 64,
            required_role="release-operator",
        )
        assert request.state == "pending"
        assert router.pending()[0].approval_id == request.approval_id
        approved = router.approve(job.job_id, approver="alice", reason="reviewed policy gate")
        assert approved.state == "approved"
        assert approved.decided_by == "alice"
        assert store.get(job.job_id).state == "queued"

        denied_job, _ = store.submit(_packet("deny-me"))
        store.claim(denied_job.job_id, "worker-a")
        denied_request = router.request(
            denied_job.job_id,
            "worker-a",
            approval_scope="dispatch blocked operation",
            request_digest="e" * 64,
        )
        denied = router.deny(denied_job.job_id, approver="bob", reason="policy not satisfied")
        assert denied.state == "denied"
        assert denied.approval_id == denied_request.approval_id
        assert router.get(denied_job.job_id).state == "denied"
        assert "api_key" not in json.dumps(store.get(denied_job.job_id).to_dict())
        with pytest.raises(BrainRunError):
            router.request(
                denied_job.job_id,
                "worker-a",
                approval_scope="again",
                request_digest="f" * 64,
            )


def test_model_health_store_aggregates_cross_process_safe_provider_observations(tmp_path):
    observations = [
        BrainModelObservation(
            provider="openai",
            model="reasoning-1",
            domain="engineering",
            capability="release_audit",
            risk_class="review",
            status="provider_refused",
            outcome="failure",
            latency_ms=20,
            failure_class="provider_error",
        )
        for _ in range(3)
    ]
    observations.append(
        BrainModelObservation(
            provider="openai",
            model="reasoning-1",
            domain="research",
            capability="literature_review",
            risk_class="review",
            status="completed",
            outcome="success",
            latency_ms=40,
            quality_reward=0.9,
            quality_passed=True,
            outcome_digest="a" * 64,
        )
    )
    with BrainModelHealthStore(tmp_path / "health.sqlite3") as health:
        for observation in observations:
            health.record(observation)
        rows = health.health()
        assert len(rows) == 1
        assert rows[0].attempts == 4
        assert rows[0].failures == 3
        assert rows[0].successes == 1
        assert rows[0].average_latency_ms == 25.0
        snapshot = health.provider_health(min_attempts=4, failure_threshold=0.75)
        assert snapshot["openai"]["circuit"] == "open"
        assert snapshot["openai"]["models"]["reasoning-1"]["failure_rate"] == 0.75
        assert health.verify_integrity()["verified"] is True
        with pytest.raises(BrainRunError):
            health.record({**observations[0].to_dict(), "api_key": "forbidden"})


def test_empty_model_health_store_is_a_valid_closed_control_plane_snapshot(tmp_path):
    with BrainModelHealthStore(tmp_path / "empty-health.sqlite3") as health:
        assert health.health() == ()
        assert health.provider_health() == {}
        assert health.verify_integrity()["verified"] is True


def test_replay_engine_runs_all_builtin_domains_and_updates_bandit_without_evidence_leak():
    registry = DomainEvaluatorRegistry.with_builtin_profiles()
    calls: list[dict[str, object]] = []

    def update(value):
        calls.append(dict(value))
        assert "evidence" not in value
        return {"next_state": {"arms": [{"arm_id": value["arm_id"], "attempts": 1, "reward_sum": value["reward"]}]}}

    report = BrainReplayEngine().replay(
        [_case(domain) for domain in ("engineering", "research", "operations", "data", "biomedical")],
        evaluators=registry,
        bandit_state={"arms": []},
        bandit_updater=update,
    )
    assert report.cases == 5
    assert report.disagreement_count == 0
    assert set(report.by_domain) == {"engineering", "research", "operations", "data", "biomedical"}
    assert all(row["passed"] for row in report.decisions)
    assert len(calls) == 5
    assert report.next_bandit_state["arms"][-1]["attempts"] == 1

    broken = _case("research")
    broken["evidence_digest"] = "0" * 64
    with pytest.raises(BrainRunError):
        BrainReplayCase.from_mapping(broken)
    secret = _case("data")
    secret["evidence"] = {"api_key": "secret"}
    secret["evidence_digest"] = _digest(secret["evidence"])
    with pytest.raises(BrainRunError):
        BrainReplayCase.from_mapping(secret)


def test_worker_claims_and_completes_a_job_across_the_control_plane(tmp_path):
    with BrainJobStore(tmp_path / "jobs.sqlite3") as store:
        job, _ = store.submit(_packet("worker-job"))
        evaluator = DomainEvaluatorRegistry.with_builtin_profiles().resolve("engineering")
        worker = BrainWorker(
            _StubBrain(),
            store,
            worker_id="worker-a",
            resolver=lambda _: {},
            evaluator=evaluator,
            bandit_state={"arms": []},
            lease_seconds=2,
            heartbeat_seconds=0.1,
        )
        result = worker.run_once(job.job_id)
        assert result is not None
        assert result.status == "succeeded"
        assert store.get(job.job_id).state == "succeeded"
        assert worker.run_once() is None


def test_worker_records_provider_transport_failures_without_persisting_provider_payloads(tmp_path):
    with BrainJobStore(tmp_path / "jobs.sqlite3") as store, BrainModelHealthStore(tmp_path / "health.sqlite3") as health:
        job, _ = store.submit(_packet("worker-provider-failure"))
        brain = _RuntimeFailureBrain()
        evaluator = DomainEvaluatorRegistry.with_builtin_profiles().resolve("engineering")
        worker = BrainWorker(
            brain,
            store,
            worker_id="worker-a",
            resolver=lambda _: {},
            evaluator=evaluator,
            bandit_state={"arms": []},
            health=health,
            lease_seconds=2,
            heartbeat_seconds=0.1,
        )
        result = worker.run_once(job.job_id)
        assert result is not None
        assert result.status == "failed"
        rows = health.health(provider="openai", model="failure-model")
        assert len(rows) == 1
        assert rows[0].failures == 1
        serialized = json.dumps(store.get(job.job_id).to_dict())
        assert "worker-secret" not in serialized
        assert "private" not in serialized


def test_worker_converts_unhandled_execution_errors_into_reconciliation_state(tmp_path):
    with BrainJobStore(tmp_path / "jobs.sqlite3") as store:
        job, _ = store.submit(_packet("worker-exception"))
        evaluator = DomainEvaluatorRegistry.with_builtin_profiles().resolve("engineering")
        worker = BrainWorker(
            _ExplodingBrain(),
            store,
            worker_id="worker-a",
            resolver=lambda _: {},
            evaluator=evaluator,
            bandit_state={"arms": []},
            lease_seconds=2,
            heartbeat_seconds=0.1,
        )
        result = worker.run_once(job.job_id)
        assert result is not None
        assert result.status == "reconciliation_required"
        assert store.get(job.job_id).side_effect_boundary == "unknown"  # type: ignore[union-attr]


def test_durable_health_snapshot_narrows_live_model_selection_without_granting_eligibility():
    credentials = CredentialStore()
    runtime = LLMRuntime(credentials)
    runtime.register_provider(openai_provider())
    handle = credentials.register("openai", "caller-secret")
    selection = AutonomousBrain(object(), runtime).build_adaptive_model_selection(
        task="select a model",
        model_candidates=[
            {
                "provider": "openai",
                "model": "reasoning-1",
                "context_window_tokens": 16_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 20,
                "cost_per_million_tokens": 1,
            }
        ],
        credentials={"openai": handle},
        selection_overrides={
            "provider_health": {
                "openai": {"circuit": "open", "historical_failure_rate": 1.0},
            }
        },
    )
    assert selection["models"][0]["enabled"] is False
    assert selection["provider_health"]["openai"]["circuit"] == "open"
    assert selection["provider_health"]["openai"]["credential_ready"] is True
    assert "caller-secret" not in json.dumps(selection)
