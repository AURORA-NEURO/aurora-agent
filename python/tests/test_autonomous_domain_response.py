from __future__ import annotations

import json
from dataclasses import replace

import pytest

from prism_sdk.autonomous_domain_response import (
    AUTONOMOUS_DOMAIN_RESPONSE_FIELDS,
    AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
    AutonomousDomainResponseContract,
    build_autonomous_domain_response_contract,
    evaluate_autonomous_domain_response,
    replay_autonomous_domain_response_evaluation,
    validate_autonomous_domain_response,
    validate_autonomous_domain_response_evaluation,
)
from prism_sdk.autonomy import (
    AutonomousAgent,
    AutonomousTaskOrchestrator,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
)
from prism_sdk.brain import AutonomousBrain, BrainLearningLedger, BrainRunResult
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime, ProviderResponse


def _response(contract: AutonomousDomainResponseContract, *, complete: bool = True) -> dict[str, object]:
    entries = {
        field: ([f"bounded {field} observation"] if complete else [])
        for field in contract.domain_fields
    }
    stages = [
        {
            "stage_id": stage_id,
            "status": "complete" if complete else "not_attempted",
            "evidence": [f"evidence for {stage_id}"] if complete else [],
            "findings": [f"finding for {stage_id}"] if complete else [],
            "uncertainty": [f"uncertainty for {stage_id}"] if complete else [],
            "open_questions": [f"question for {stage_id}"] if complete else [],
        }
        for stage_id in contract.stage_ids
    ]
    return {
        "schema": AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
        "domain": contract.domain,
        "workflow_id": contract.workflow_id,
        "status": "complete" if complete else "partial",
        "answer": "A bounded answer with explicit evidence and uncertainty." if complete else "A partial answer.",
        "observations": ["one observed provider statement"] if complete else [],
        "inferences": ["one bounded inference"] if complete else [],
        "uncertainty": ["one unresolved uncertainty"] if complete else [],
        "evidence_gaps": ["one evidence gap"] if complete else [],
        "next_actions": ["one caller-approved next action"] if complete else [],
        "stages": stages,
        "domain_details": entries,
        "retention": "transient_provider_response_only;validated_against_reviewed_domain_contract",
        "secret_material": "never_returned",
    }


def _contract_for(profile: object, domain: str) -> AutonomousDomainResponseContract:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == domain)
    return build_autonomous_domain_response_contract(profile, workflow=workflow)


class _LearningWorkspace:
    def __init__(self) -> None:
        self.calls: list[dict[str, object]] = []

    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        assert name == "brain_outcome_record"
        payload = {} if arguments is None else dict(arguments)
        self.calls.append(payload)
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


def _structured_result(contract: AutonomousDomainResponseContract, run_id: str) -> BrainRunResult:
    return BrainRunResult(
        run_id=run_id,
        status="completed_provider_call",
        selection={
            "selected_model": {"provider": "offline", "model": "fixture"},
            "decision_digest": "a" * 64,
            "context_digest": "b" * 64,
        },
        prompt={"prompt_digest": "c" * 64},
        plan={"plan": {"plan_digest": "d" * 64}},
        response=ProviderResponse(
            provider="offline",
            model="fixture",
            text="bounded structured response",
            status_code=200,
            request_id=run_id,
            usage={},
            raw={},
            structured=_response(contract),
        ),
        outcome_digest="e" * 64,
    )


def test_contract_covers_every_builtin_domain_and_replays() -> None:
    profiles = builtin_autonomous_domain_profiles()
    assert {profile.domain for profile in profiles} == set(AUTONOMOUS_DOMAIN_RESPONSE_FIELDS)

    for profile in profiles:
        contract = _contract_for(profile, profile.domain)
        response = _response(contract)
        normalized = validate_autonomous_domain_response(response, contract)
        evaluation = evaluate_autonomous_domain_response(normalized.to_dict(), contract)
        assert evaluation.passed is True
        assert evaluation.reward == 1.0
        assert validate_autonomous_domain_response_evaluation(evaluation.to_dict()).to_dict() == evaluation.to_dict()
        assert replay_autonomous_domain_response_evaluation(response, contract, evaluation).evaluation_digest == evaluation.evaluation_digest


def test_structured_blueprint_binds_prompt_and_schema_to_workflow() -> None:
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    blueprint = orchestrator.prepare(
        task="inspect a delivery change",
        domain="coding",
        structured_domain_response=True,
    )
    assert blueprint.spec.structured_domain_response is True
    assert blueprint.spec.require_json is True
    assert blueprint.response_contract is not None
    assert blueprint.spec.response_schema == blueprint.response_contract.response_schema
    assert any(chunk["id"] == "autonomy-domain-response-contract" for chunk in blueprint.prompt["context"])
    assert blueprint.to_dict()["response_contract"]["contract_digest"] == blueprint.response_contract.contract_digest


def test_response_contract_is_strict_and_never_accepts_credential_shaped_values() -> None:
    contract = _contract_for(
        next(profile for profile in builtin_autonomous_domain_profiles() if profile.domain == "operations"),
        "operations",
    )
    malformed = _response(contract)
    malformed["stages"] = list(malformed["stages"])  # type: ignore[arg-type]
    malformed["stages"][0] = dict(malformed["stages"][0])  # type: ignore[index]
    malformed["stages"][0]["extra"] = "not permitted"  # type: ignore[index]
    with pytest.raises(ArgumentError):
        validate_autonomous_domain_response(malformed, contract)

    unsafe = _response(contract)
    unsafe["answer"] = "provider said gsk_fixture_redacted"
    with pytest.raises(ArgumentError):
        validate_autonomous_domain_response(unsafe, contract)


def test_replay_rejects_changed_structural_feedback() -> None:
    profile = builtin_autonomous_domain_profiles()[0]
    contract = _contract_for(profile, profile.domain)
    response = _response(contract)
    evaluation = evaluate_autonomous_domain_response(response, contract)
    changed = _response(contract, complete=False)
    with pytest.raises(ArgumentError):
        replay_autonomous_domain_response_evaluation(changed, contract, evaluation)


def test_structured_evaluation_validator_rejects_digest_or_secret_tampering() -> None:
    profile = builtin_autonomous_domain_profiles()[0]
    contract = _contract_for(profile, profile.domain)
    evaluation = evaluate_autonomous_domain_response(_response(contract), contract).to_dict()
    changed = dict(evaluation)
    changed["evaluation_digest"] = "0" * 64
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_domain_response_evaluation(changed)
    unsafe = dict(evaluation)
    unsafe["replan_instruction"] = "send gsk_fixture_redacted"
    with pytest.raises(ArgumentError, match="credential"):
        validate_autonomous_domain_response_evaluation(unsafe)


def test_provider_boundary_attaches_value_only_evaluation_to_result() -> None:
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    blueprint = orchestrator.prepare(
        task="inspect a delivery change",
        domain="coding",
        structured_domain_response=True,
    )
    response = ProviderResponse(
        provider="offline",
        model="fixture",
        text="bounded structured response",
        status_code=200,
        request_id=None,
        usage={},
        raw={},
        structured=_response(blueprint.response_contract),  # type: ignore[arg-type]
    )
    result = BrainRunResult(
        run_id="structured-response-boundary",
        status="completed_provider_call",
        selection={},
        prompt={},
        plan={},
        response=response,
        outcome_digest="a" * 64,
    )
    attached = orchestrator._attach_domain_response_evaluation(blueprint, result)
    assert attached.response_evaluation is not None  # type: ignore[union-attr]
    assert attached.response_evaluation["passed"] is True  # type: ignore[union-attr]
    assert "response_evaluation" in attached.to_dict()  # type: ignore[union-attr]


def test_structured_response_settlement_is_restart_safe_for_every_domain(tmp_path) -> None:
    workspace = _LearningWorkspace()
    ledger = BrainLearningLedger(tmp_path / "structured-response-learning.jsonl")
    agent = AutonomousAgent(workspace, LLMRuntime(), ledger=ledger)
    bandit_state = {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}
    for index, profile in enumerate(builtin_autonomous_domain_profiles()):
        contract = _contract_for(profile, profile.domain)
        result = _structured_result(contract, f"structured-{profile.domain}")
        attached = agent.orchestrator._attach_domain_response_evaluation(
            agent.orchestrator.prepare(
                task=f"validate {profile.domain}",
                domain=profile.domain,
                structured_domain_response=True,
            ),
            result,
        )
        evaluation = validate_autonomous_domain_response_evaluation(attached.response_evaluation)  # type: ignore[arg-type]
        episode = agent.prepare_learning_episode(attached, ledger=ledger)
        saved = json.loads(json.dumps(episode.to_dict()))
        metadata_only = replace(attached, response=None)
        restored_agent = AutonomousAgent(workspace, LLMRuntime(), ledger=ledger)
        decision, report = restored_agent.settle_structured_response(
            metadata_only,
            episode=saved,
            bandit_state=bandit_state,
            ledger=ledger,
        )
        assert decision.evaluator_id == f"autonomous-{profile.domain}-response-integrity"
        assert report["structured_response"]["evaluation_digest"] == evaluation.evaluation_digest
        assert ledger.pending_episodes() == []
        assert "bounded structured response" not in json.dumps(ledger.records())
        bandit_state = report["next_state"]
        assert isinstance(bandit_state, dict)
        assert bandit_state["generation"] == index + 1
