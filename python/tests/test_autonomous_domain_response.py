from __future__ import annotations

import pytest

from prism_sdk.autonomous_domain_response import (
    AUTONOMOUS_DOMAIN_RESPONSE_FIELDS,
    AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
    AutonomousDomainResponseContract,
    build_autonomous_domain_response_contract,
    evaluate_autonomous_domain_response,
    replay_autonomous_domain_response_evaluation,
    validate_autonomous_domain_response,
)
from prism_sdk.autonomy import (
    AutonomousTaskOrchestrator,
    builtin_autonomous_domain_profiles,
    builtin_autonomous_workflow_strategies,
)
from prism_sdk.brain import AutonomousBrain
from prism_sdk.brain import BrainRunResult
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
