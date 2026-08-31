from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    ArgumentError,
    AutonomousAgent,
    AutonomousWorkflowPortfolioExecutionCheckpoint,
    AutonomousWorkflowPortfolioExecutionItem,
    AutonomousWorkflowPortfolioExecutionResult,
    AutonomousWorkflowPortfolioEvidenceController,
    AutonomousWorkflowPortfolioEvidenceItemRequest,
    InMemoryAutonomousEvidenceRuntimeJournal,
    InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore,
    JsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence,
    LLMRuntime,
    content_digest,
    execute_autonomous_workflow_portfolio_evidence,
    execute_autonomous_workflow_portfolio_evidence_resumable,
    validate_autonomous_workflow_portfolio_evidence_checkpoint,
)
from prism_sdk.autonomous_workflow_portfolio import AutonomousWorkflowPortfolioPlan


def _agent() -> AutonomousAgent:
    return AutonomousAgent(object(), LLMRuntime())


def _requests(domains=AUTONOMOUS_DOMAINS, *, chain: bool = False) -> list[dict[str, object]]:
    result = []
    for index, domain in enumerate(domains):
        item: dict[str, object] = {
            "id": f"item-{index}-{domain}",
            "task": f"perform a bounded {domain} evidence review",
            "domain": domain,
        }
        if chain and index:
            item["depends_on"] = [f"item-{index - 1}-{domains[index - 1]}"]
        result.append(item)
    return result


def _provider_execution(
    agent: AutonomousAgent,
    domains=AUTONOMOUS_DOMAINS,
    *,
    chain: bool = False,
    requests_override=None,
):
    requests = _requests(domains, chain=chain) if requests_override is None else requests_override
    plan = AutonomousWorkflowPortfolioPlan.from_dict(
        agent.plan_workflow_portfolio(requests, require_all_domains=False)
    )
    providers = tuple(
        AutonomousWorkflowPortfolioExecutionItem(
            item_id=item.item_id,
            domain=item.domain,
            depends_on=item.depends_on,
            status="succeeded",
            result_digest=content_digest({"provider_result": item.item_id}),
            result_bytes=32,
        )
        for item in plan.items
    )
    item_ids = tuple(item.item_id for item in plan.items)
    checkpoint = AutonomousWorkflowPortfolioExecutionCheckpoint.create(
        job_id="provider-fixture",
        plan_digest=plan.portfolio_digest,
        portfolio_input_digest=content_digest(requests),
        item_ids=item_ids,
        request_digests=tuple(item.request_digest for item in plan.items),
        task_digests=tuple(item.task_digest for item in plan.items),
        settled_item_ids=tuple(sorted(item_ids)),
        settled_result_digests=tuple(
            item.result_digest for item in sorted(providers, key=lambda value: value.item_id)
        ),
        max_parallelism=4,
        stop_on_error=False,
        status="completed",
    )
    execution = AutonomousWorkflowPortfolioExecutionResult(
        status="completed",
        plan=plan,
        items=providers,
        executed_waves=plan.dependency_graph.waves,
        completed_count=len(providers),
        failed_count=0,
        blocked_count=0,
        approval_required_count=0,
        next_action="complete",
        checkpoint=checkpoint,
    )
    return execution, requests


class _Adapters:
    evaluator_id = "portfolio-fixture-evaluator"
    evaluator_version = "2026.08"

    def __init__(self) -> None:
        self.acquisition_calls: list[str] = []
        self.parent_digests: list[tuple[str, ...]] = []

    def acquire(self, context):
        requirement = context["requirement"]
        self.acquisition_calls.append(requirement.requirement_id)
        self.parent_digests.append(tuple(context["parent_evidence_digests"]))
        return {
            "fixture": "transient-value",
            "requirement_id": requirement.requirement_id,
        }

    def project(self, _value, context):
        return [
            {
                "label": context["requirement"].label,
                "kind": "fact",
                "status": "observed",
                "confidence": 0.9,
            }
        ]

    def evaluate(self, _input_value):
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1.0,
        }


def _evidence_requests(agent: AutonomousAgent, domains, *, all_requirements: bool = True):
    requests = []
    for index, domain in enumerate(domains):
        scoped = agent.evidence_plan((domain,))
        requirements = scoped.requirements if all_requirements else scoped.requirements[:1]
        requests.append(
            AutonomousWorkflowPortfolioEvidenceItemRequest(
                item_id=f"item-{index}-{domain}",
                requests=tuple(
                    {
                        "requirement_id": requirement.requirement_id,
                        "source_id": f"fixture-source-{index}-{requirement.stage_id}",
                        "source_digest": None,
                        "request_id": f"fixture-request-{index}-{offset}",
                        "metadata": {"fixture": True},
                    }
                    for offset, requirement in enumerate(requirements)
                ),
            )
        )
    return requests


def test_portfolio_evidence_completes_all_builtin_domains_and_redacts_wire_metadata():
    agent = _agent()
    execution, _ = _provider_execution(agent, chain=True)
    adapters = _Adapters()
    progress = []

    result = execute_autonomous_workflow_portfolio_evidence(
        agent,
        execution,
        items=_evidence_requests(agent, AUTONOMOUS_DOMAINS),
        runtime={"acquirer": adapters, "projector": adapters, "evaluator": adapters},
        max_parallelism=4,
        progress_sink=progress.append,
    )

    assert result.status == "completed"
    assert len(result.items) == len(AUTONOMOUS_DOMAINS)
    assert all(item.status == "completed" for item in result.items)
    assert len(adapters.acquisition_calls) == len(
        result.runtime_for(result.items[0].item_id).plan.requirements
    ) + sum(
        len(result.runtime_for(item.item_id).plan.requirements)
        for item in result.items[1:]
    )
    assert len(progress) == len(AUTONOMOUS_DOMAINS)
    wire = json.dumps(result.to_dict(), sort_keys=True)
    assert "transient-value" not in wire
    assert result.to_dict()["retention"] == "metadata_only;raw_evidence_values_caller_owned"
    assert all(item.runtime is not None and item.runtime.values for item in result.items)
    assert any(adapters.parent_digests)


def test_portfolio_evidence_keeps_provider_failures_and_dependency_omissions_explicit():
    agent = _agent()
    execution, _ = _provider_execution(
        agent,
        ("coding", "data", "science"),
        requests_override=[
            {"id": "item-0-coding", "task": "perform a bounded coding evidence review", "domain": "coding"},
            {"id": "item-1-data", "task": "perform a bounded data evidence review", "domain": "data", "depends_on": ["item-0-coding"]},
            {"id": "item-2-science", "task": "perform a bounded science evidence review", "domain": "science"},
        ],
    )
    providers = list(execution.items)
    failed = providers[0]
    providers[0] = AutonomousWorkflowPortfolioExecutionItem(
        item_id=failed.item_id,
        domain=failed.domain,
        depends_on=failed.depends_on,
        status="failed",
        error_class="provider_fixture_failed",
    )
    execution = AutonomousWorkflowPortfolioExecutionResult(
        status="partial",
        plan=execution.plan,
        items=tuple(providers),
        executed_waves=execution.executed_waves,
        completed_count=2,
        failed_count=1,
        blocked_count=0,
        approval_required_count=0,
        next_action="reconcile",
        checkpoint=execution.checkpoint,
    )

    result = execute_autonomous_workflow_portfolio_evidence(
        agent,
        execution,
        items=_evidence_requests(agent, ("coding", "data", "science")),
        runtime={"acquirer": _Adapters()},
    )

    assert result.items[0].status == "omitted"
    assert result.items[0].error_class == "provider_execution_not_succeeded"
    assert result.items[1].status == "omitted"
    assert result.items[1].error_class == "evidence_dependency_not_completed"
    assert result.items[2].status == "partial"


def test_resumable_portfolio_evidence_replays_journals_without_reacquisition():
    agent = _agent()
    execution, _ = _provider_execution(agent, ("coding", "data", "science"), chain=True)
    evidence_items = _evidence_requests(agent, ("coding", "data", "science"))
    journals = {item.item_id: InMemoryAutonomousEvidenceRuntimeJournal() for item in evidence_items}
    first_adapters = _Adapters()
    checkpoints = []

    def journal_for(item_id, **_context):
        return journals[item_id]

    first = execute_autonomous_workflow_portfolio_evidence_resumable(
        agent,
        execution,
        job_id="evidence-job",
        items=evidence_items,
        runtime={"acquirer": first_adapters, "projector": first_adapters, "evaluator": first_adapters},
        checkpoint_sink=checkpoints.append,
        require_admission=False,
        journal_for=journal_for,
        max_parallelism=1,
    )
    assert first.status == "completed"
    assert checkpoints[-1].status == "completed"
    assert len(checkpoints) == len(execution.plan.items)

    values = {
        receipt.request_digest: next(
            value
            for item in first.items
            if item.runtime is not None
            for digest, value in item.runtime.values.items()
            if digest == receipt.request_digest
        )
        for item in first.items
        if item.runtime is not None
        for receipt in item.runtime.receipts
    }

    class _NoReplayAcquirer:
        def acquire(self, _context):
            raise AssertionError("a completed journal item must not reacquire evidence")

    second = execute_autonomous_workflow_portfolio_evidence_resumable(
        agent,
        execution,
        job_id="evidence-job",
        items=evidence_items,
        runtime={
            "acquirer": _NoReplayAcquirer(),
            "projector": first_adapters,
            "evaluator": first_adapters,
            "rehydrate_value": lambda receipt: values[receipt["request_digest"]],
        },
        checkpoint=checkpoints[-1],
        checkpoint_sink=checkpoints.append,
        require_admission=False,
        journal_for=journal_for,
        max_parallelism=1,
    )

    assert second.status == "completed"
    assert len(first_adapters.acquisition_calls) == sum(
        len(item.runtime.receipts) for item in first.items if item.runtime is not None
    )
    assert checkpoints[-1].to_dict() == validate_autonomous_workflow_portfolio_evidence_checkpoint(
        checkpoints[-1].to_dict()
    ).to_dict()


def test_portfolio_evidence_checkpoint_storage_controller_and_tamper_fencing():
    agent = _agent()
    execution, _ = _provider_execution(agent, ("coding",))
    evidence_items = _evidence_requests(agent, ("coding",))
    store = InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore()
    controller = AutonomousWorkflowPortfolioEvidenceController(
        agent,
        execution,
        job_id="controller-job",
        persistence=store,
        require_admission=False,
    )
    adapters = _Adapters()
    result = controller.run(
        items=evidence_items,
        runtime={"acquirer": adapters, "projector": adapters, "evaluator": adapters},
        journal_for=lambda **_context: InMemoryAutonomousEvidenceRuntimeJournal(),
    )
    assert result.status == "completed"
    assert controller.projection()["status"] == "completed"

    class _TextStore:
        def __init__(self):
            self.value = None

        def read(self):
            return self.value

        def write(self, value):
            self.value = value

    text_store = _TextStore()
    persistence = JsonAutonomousWorkflowPortfolioEvidenceCheckpointPersistence(text_store)
    persistence.write(store.read())
    assert persistence.read() == store.read()

    tampered = dict(store.read())
    tampered["evidence_input_digest"] = "0" * 64
    with pytest.raises(ArgumentError):
        validate_autonomous_workflow_portfolio_evidence_checkpoint(tampered)
