from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_AUTHORIZATION_OPERATIONS,
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
    AutonomousAuthorizationError,
    AutonomousRunTraceSession,
    InMemoryAutonomousRunTraceStore,
)
from prism_sdk.brain import AutonomousBrain, BrainOutcomeEvaluator, BrainRunResult
from prism_sdk.llm_runtime import LLMRuntime, ProviderResponse
from prism_sdk.memory import BrainEpisodicMemory


class _Workspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        assert name == "brain_outcome_record"
        payload = {} if arguments is None else arguments
        state = payload["bandit_state"]
        assert isinstance(state, dict)
        return {
            "ok": True,
            "status": "recorded_evaluator_reward",
            "next_state": {**state, "generation": int(state.get("generation", 0)) + 1},
            "learning_evidence": {
                "schema": "bioprism-brain-learning-evidence/0.1",
                "evidence_digest": "f" * 64,
            },
        }


def _context(operations: tuple[str, ...], *, max_uses: int | None = None):
    ledger = AutonomousAuthorizationLedger(max_grants=4, max_events=512)
    grant = ledger.issue(
        grant_id="boundary-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=AUTONOMOUS_DOMAIN_NAMES,
        allowed_operations=operations,
        allowed_capabilities=(),
        allowed_risk_classes=(),
        issued_at=1_000,
        expires_at=100_000,
        max_uses=max_uses,
    )
    return ledger, AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=grant.authorization_digest,
        domains=AUTONOMOUS_DOMAIN_NAMES,
        risk_class=None,
        request_prefix="boundary",
        clock=lambda: 2_000,
    )


def _result() -> BrainRunResult:
    context = {
        "domain": "coding",
        "capability": "analysis",
        "risk_class": "read_only",
        "task_family": None,
    }
    context_digest = hashlib.sha256(
        json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    return BrainRunResult(
        run_id="boundary-run",
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "offline", "model": "model-a"},
            "decision_digest": "a" * 64,
            "context_digest": context_digest,
            "context": context,
            "selection_audit": {},
        },
        prompt={"prompt_digest": "b" * 64},
        plan={"plan": {"plan_digest": "c" * 64}},
        response=ProviderResponse(
            provider="offline",
            model="model-a",
            text="transient response",
            status_code=200,
            request_id="request-1",
            usage={"total_tokens": 1},
            raw={},
        ),
        outcome_digest="d" * 64,
    )


def test_trace_write_is_authorized_for_every_builtin_domain() -> None:
    ledger, context = _context(("trace_write",), max_uses=None)
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 2_000)
    session = AutonomousRunTraceSession(
        store,
        run_id="boundary-trace",
        task_digest="e" * 64,
        domains=AUTONOMOUS_DOMAIN_NAMES,
        authorization_context=context,
    )
    session.started()
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        session.record(phase="plan_compiled", status="running", domains=(domain,))
    session.complete(status="completed")
    assert len(ledger.events()) == len(AUTONOMOUS_DOMAIN_NAMES) + 3  # grant issuance + trace writes


def test_memory_and_learning_boundaries_authorize_before_their_sinks(tmp_path) -> None:
    ledger, context = _context(("memory_retrieval", "memory_write", "evaluation", "learning"), max_uses=None)
    memory = BrainEpisodicMemory(tmp_path / "memory.sqlite3")
    brain = AutonomousBrain(_Workspace(), LLMRuntime(), memory)
    result = _result()

    assert brain.recall_memory({"domain": "coding"}, authorization_context=context) == []
    brain.remember_result(
        result,
        task="inspect a bounded change",
        context={"domain": "coding", "capability": "analysis", "risk_class": "read_only"},
        authorization_context=context,
    )
    evaluator = BrainOutcomeEvaluator(
        lambda _: {"reward": 0.75, "passed": True},
        evaluator_id="boundary-evaluator",
        evaluator_version="1",
        authorization_context=context,
    )
    episode = brain.prepare_learning_episode(result)
    evaluator.evaluate_episode(brain, episode, bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []})
    assert len(ledger.events()) == 5  # grant issuance + four authorized boundary writes


def test_authorization_operation_catalogue_remains_complete() -> None:
    assert set(AUTONOMOUS_AUTHORIZATION_OPERATIONS) == {
        "plan",
        "provider_invocation",
        "evidence_acquisition",
        "connector_dispatch",
        "tool_execution",
        "effect_dispatch",
        "evaluation",
        "learning",
        "memory_retrieval",
        "memory_write",
        "trace_write",
        "analytics_write",
    }


def test_mission_learning_cycle_cannot_bypass_memory_retrieval_authorization(tmp_path) -> None:
    _ledger, context = _context(("memory_write", "evaluation", "learning"), max_uses=None)
    memory = BrainEpisodicMemory(tmp_path / "mission-memory.sqlite3")
    brain = AutonomousBrain(_Workspace(), LLMRuntime(), memory)
    evaluator = BrainOutcomeEvaluator(
        lambda _input: {"reward": 0.0, "passed": False, "failed": True},
        evaluator_id="boundary-evaluator",
        evaluator_version="1",
    )

    with pytest.raises(AutonomousAuthorizationError):
        brain.run_adaptive_mission_learning_cycle(
            task="inspect a bounded mission",
            model_candidates=[],
            prompt={"max_input_tokens": 100},
            plan={"allowed_tools": ["provider.invoke"]},
            credentials={},
            mission_policy={"allowed_tools": ["developer_platform_status"]},
            evaluator=evaluator,
            bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
            memory=memory,
            authorization_context=context,
            authorization_domain="coding",
        )
    assert memory.retrieve({"limit": 8}) == []
    memory.close()
