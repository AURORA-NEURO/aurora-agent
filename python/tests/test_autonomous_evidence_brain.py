from __future__ import annotations

import json

import pytest

from test_autonomy import _Workspace, _model, _runtime

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousEvidenceBackedController,
    InMemoryAutonomousEvidenceBackedCheckpointStore,
    InMemoryAutonomousEvidenceRuntimeJournal,
    JsonAutonomousEvidenceBackedCheckpointPersistence,
    ModelCatalogue,
    TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence,
    AutonomousEvidenceBackedCheckpoint,
    content_digest,
    validate_autonomous_evidence_backed_checkpoint,
)
from prism_sdk.errors import ArgumentError


class _AcceptAllEvidence:
    evaluator_id = "fixture-evaluator"
    evaluator_version = "v1"

    def evaluate(self, _input: object) -> dict[str, object]:
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


def _requests(agent: AutonomousAgent, domains: tuple[str, ...]) -> list[dict[str, object]]:
    plan = agent.evidence_plan(domains)
    return [
        {
            "requirement_id": requirement.requirement_id,
            "source_id": "credentialless-fixture",
            "request_id": f"fixture-{index}",
            "metadata": {"fixture": "deterministic"},
        }
        for index, requirement in enumerate(plan.requirements)
    ]


def test_reviewed_evidence_bridge_requires_source_approval_before_acquisition():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "evidence-approval-test")
    calls: list[object] = []

    try:
        result = agent.run_with_reviewed_evidence(
            task="review every autonomous domain",
            domains=AUTONOMOUS_DOMAINS,
            requests=_requests(agent, AUTONOMOUS_DOMAINS),
            acquirer=lambda context: calls.append(context) or {"private_payload": "caller-owned"},
            credentials={"openai": handle},
            model_candidates=_model(),
            approve_source_dispatch=False,
        )
        assert result.status == "evidence_review_required"
        assert result.evidence is None
        assert calls == []
        assert "caller-owned" not in json.dumps(result.to_dict())
        assert result.to_dict()["evidence_plan_digest"] == result.evidence_plan.plan_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_executes_every_builtin_domain_and_redacts_transient_values():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "all-domain-evidence-test")

    try:
        for domain in AUTONOMOUS_DOMAINS:
            domains = (domain,)
            plan = agent.evidence_plan(domains)
            requests = _requests(agent, domains)

            def acquirer(context: object, *, selected_domain: str = domain) -> dict[str, str]:
                requirement = context["requirement"]  # type: ignore[index]
                return {
                    "private_payload": f"caller-owned-{selected_domain}",
                    "requirement": requirement.requirement_id,
                }

            def projector(value: object, context: object) -> list[dict[str, object]]:
                _ = value
                requirement = context["requirement"]  # type: ignore[index]
                return [{"label": requirement.label}]

            result = agent.run_with_reviewed_evidence(
                task=f"review a bounded {domain} task",
                domains=domains,
                requests=requests,
                acquirer=acquirer,
                projector=projector,
                evaluator=_AcceptAllEvidence(),
                credentials={"openai": handle},
                model_candidates=_model(),
                run_mode="domain",
                approve_source_dispatch=True,
                approve_provider_call=True,
            )
            assert result.status == "completed", domain
            assert result.evidence is not None
            assert result.evidence.status == "completed", domain
            assert result.execution_status == "completed_provider_call", domain
            projection = result.to_dict()
            assert projection["schema"] == "bioprism-python-autonomous-evidence-backed-run/0.1"
            assert f"caller-owned-{domain}" not in json.dumps(projection)
            assert "bounded" not in json.dumps(projection)
            assert projection["evidence_plan_digest"] == plan.plan_digest
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_replays_with_rehydration_and_keeps_provider_approval_separate():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "evidence-replay-test")
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    values: dict[str, object] = {}
    domains = ("science",)
    requests = _requests(agent, domains)

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        value = {
            "private_payload": "replay-me-only-in-memory",
            "requirement": requirement.requirement_id,
        }
        values[content_digest(value)] = value
        return value

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        pending_provider = agent.run_with_reviewed_evidence(
            task="replay an accepted science evidence run",
            domains=domains,
            requests=requests,
            acquirer=acquirer,
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            journal=journal,
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=False,
        )
        assert pending_provider.status == "provider_review_required"
        assert pending_provider.execution_status == "approval_required"
        assert getattr(server, "request_count", 0) == 0

        completed = agent.run_with_reviewed_evidence(
            task="replay an accepted science evidence run",
            domains=domains,
            requests=requests,
            acquirer=lambda _context: (_ for _ in ()).throw(AssertionError("replay reacquired evidence")),
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            journal=journal,
            rehydrate_value=lambda receipt: values.get(receipt["value_digest"]),
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
        )
        assert completed.status == "completed"
        assert completed.evidence is not None
        assert all(receipt.replay == "replayed" for receipt in completed.evidence.receipts)
        assert getattr(server, "request_count", 0) > 0
        assert "replay-me-only-in-memory" not in json.dumps(completed.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_reviewed_evidence_supports_bounded_cross_domain_invocation_and_transient_prompt_projection():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "cross-domain-evidence-test")
    domains = ("coding", "science")

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        return {
            "prompt_only": "transient-cross-domain-value",
            "requirement": requirement.requirement_id,
        }

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def prompt_builder(evidence: object) -> dict[str, object]:
        values = evidence.values  # type: ignore[attr-defined]
        return {
            "reviewed_evidence": {
                "status": evidence.status,  # type: ignore[attr-defined]
                "transient_value": next(iter(values.values())),
            }
        }

    try:
        result = agent.run_with_reviewed_evidence(
            task="reconcile coding and science findings",
            domains=domains,
            requests=_requests(agent, domains),
            acquirer=acquirer,
            projector=projector,
            evaluator=_AcceptAllEvidence(),
            prompt_builder=prompt_builder,
            credentials={"openai": handle},
            model_candidates=_model(),
            run_mode="cross_domain",
            approve_source_dispatch=True,
            approve_provider_call=True,
        )
        assert result.status == "completed"
        assert result.execution_status == "completed"
        assert result.prompt_context["reviewed_evidence"]["transient_value"]["prompt_only"] == "transient-cross-domain-value"  # type: ignore[index]
        assert "transient-cross-domain-value" not in json.dumps(result.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_evidence_controller_replays_sources_and_provider_results_without_dispatch():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-evidence-test")
    checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    controller = AutonomousEvidenceBackedController(agent, "resumable-evidence-job", checkpoint_store)
    values: dict[str, object] = {}
    acquisitions = 0
    domains = ("coding",)
    requests = _requests(agent, domains)

    def acquirer(context: object) -> dict[str, str]:
        nonlocal acquisitions
        acquisitions += 1
        requirement = context["requirement"]  # type: ignore[index]
        value = {
            "private_payload": "resumable-caller-value",
            "requirement": requirement.requirement_id,
        }
        values[content_digest(value)] = value
        return value

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    def run_options(*, provider: bool = False) -> dict[str, object]:
        return {
            "requests": requests,
            "acquirer": acquirer,
            "projector": projector,
            "evaluator": _AcceptAllEvidence(),
            "journal": journal,
            "rehydrate_value": lambda receipt: values.get(receipt["value_digest"]),
            "credentials": {"openai": handle},
            "model_candidates": _model(),
            "domains": domains,
            "run_mode": "domain",
            "approve_source_dispatch": True,
            "approve_provider_call": provider,
        }

    try:
        assert controller.restore()["status"] == "empty"
        first = controller.run(task="resume a coding evidence run", **run_options())
        assert first["run"].status == "provider_pending"
        assert first["run"].checkpoint.status == "provider_pending"
        assert acquisitions == len(requests)
        assert getattr(server, "request_count", 0) == 0

        second = controller.run(task="resume a coding evidence run", **run_options(provider=True))
        assert second["run"].status == "completed"
        assert second["run"].checkpoint.status == "completed"
        assert acquisitions == len(requests), "source values must replay from the journal"
        provider_calls = getattr(server, "request_count", 0)
        assert provider_calls > 0
        provider_result = second["run"].result.execution
        assert provider_result is not None

        third = controller.run(
            task="resume a coding evidence run",
            **run_options(),
            rehydrate_provider_run=lambda _checkpoint, _probe: provider_result,
        )
        assert third["run"].status == "completed"
        assert third["run"].provider_rehydrated is True
        assert acquisitions == len(requests)
        assert getattr(server, "request_count", 0) == provider_calls
        assert "resumable-caller-value" not in json.dumps(third["run"].to_dict())
        assert "resumable-caller-value" not in json.dumps(third["run"].checkpoint.to_dict())
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_evidence_checkpoint_is_tamper_evident_and_covers_every_domain():
    runtime, store, server, thread = _runtime()
    agent = AutonomousAgent(_Workspace(), runtime, model_catalogue=ModelCatalogue(_model()))
    handle = store.register("openai", "resumable-all-domain-test")

    def acquirer(context: object) -> dict[str, str]:
        requirement = context["requirement"]  # type: ignore[index]
        return {"requirement": requirement.requirement_id}

    def projector(value: object, context: object) -> list[dict[str, object]]:
        _ = value
        requirement = context["requirement"]  # type: ignore[index]
        return [{"label": requirement.label}]

    try:
        saved_checkpoint: dict[str, object] | None = None
        for domain in AUTONOMOUS_DOMAINS:
            checkpoint_store = InMemoryAutonomousEvidenceBackedCheckpointStore()
            controller = AutonomousEvidenceBackedController(agent, f"job-{domain}", checkpoint_store)
            journal = InMemoryAutonomousEvidenceRuntimeJournal()
            result = controller.run(
                task=f"prepare a resumable {domain} review",
                requests=_requests(agent, (domain,)),
                acquirer=acquirer,
                projector=projector,
                evaluator=_AcceptAllEvidence(),
                journal=journal,
                credentials={"openai": handle},
                model_candidates=_model(),
                domains=(domain,),
                run_mode="domain",
                approve_source_dispatch=True,
                approve_provider_call=False,
            )
            assert result["run"].status == "provider_pending", domain
            assert result["run"].checkpoint.evidence_plan_digest == agent.evidence_plan((domain,)).plan_digest
            if saved_checkpoint is None:
                saved_checkpoint = result["run"].checkpoint.to_dict()

        assert saved_checkpoint is not None
        tampered = dict(saved_checkpoint)
        tampered["status"] = "completed"
        with pytest.raises(Exception, match="completed evidence-backed checkpoint"):
            validate_autonomous_evidence_backed_checkpoint(tampered)
    finally:
        server.shutdown()
        thread.join(timeout=2)
        server.server_close()


def test_resumable_evidence_json_and_cas_persistence_are_canonical_and_fenced():
    class TextStore:
        def __init__(self) -> None:
            self.value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            observed = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
            if observed != expected:
                return False
            self.value = value
            return True

    checkpoint = AutonomousEvidenceBackedCheckpoint(
        job_id="json-persistence-job",
        task_digest="a" * 64,
        request_digest="b" * 64,
        run_policy_digest="c" * 64,
        evidence_plan_digest="d" * 64,
        execution_plan_digest="e" * 64,
        evidence_result_digest="f" * 64,
        prompt_projection_digest=None,
        provider_result_digest=None,
        provider_status=None,
        status="provider_pending",
    )
    plain_store = TextStore()
    persistence = JsonAutonomousEvidenceBackedCheckpointPersistence(plain_store)
    persistence.write(checkpoint)
    assert persistence.read() == checkpoint.to_dict()

    cas_store = TextStore()
    cas = TransactionalJsonAutonomousEvidenceBackedCheckpointPersistence(cas_store)
    assert cas.write_if_unchanged(None, checkpoint.to_dict()) is True
    assert cas.write_if_unchanged(None, checkpoint.to_dict()) is False
    assert cas.write_if_unchanged(checkpoint.checkpoint_digest, checkpoint.to_dict()) is True

    tampered = dict(checkpoint.to_dict())
    tampered["status"] = "completed"
    with pytest.raises(ArgumentError):
        validate_autonomous_evidence_backed_checkpoint(tampered)
