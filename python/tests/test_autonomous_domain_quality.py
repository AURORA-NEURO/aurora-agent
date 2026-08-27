from __future__ import annotations

from prism_sdk.autonomous_domain_quality import (
    AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD,
    assert_autonomous_domain_quality_policy_coverage,
    autonomous_domain_quality_policy,
    autonomous_domain_quality_prompt,
    builtin_autonomous_domain_quality_policies,
    evaluate_autonomous_domain_response_quality,
    validate_autonomous_domain_quality_policy,
)
from prism_sdk.autonomous_domain_response import (
    AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
    build_autonomous_domain_response_contract,
    validate_autonomous_domain_response,
)
from prism_sdk.autonomy import builtin_autonomous_domain_profiles, builtin_autonomous_workflow_strategies


def _contract(domain: str):
    profile = next(item for item in builtin_autonomous_domain_profiles() if item.domain == domain)
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == domain)
    return build_autonomous_domain_response_contract(profile, workflow=workflow)


def _response(contract):
    return {
        "schema": AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
        "domain": contract.domain,
        "workflow_id": contract.workflow_id,
        "status": "complete",
        "answer": f"A bounded {contract.domain} answer.",
        "observations": ["Observed input was inspected."],
        "inferences": ["This is a bounded inference from the observed input."],
        "uncertainty": ["External-world validation remains caller-owned."],
        "evidence_gaps": ["No unprovided source was treated as evidence."],
        "next_actions": ["Review the evidence and approve any requested effect."],
        "stages": [
            {
                "stage_id": stage_id,
                "status": "complete",
                "evidence": [f"evidence:{stage_id}"],
                "findings": [f"finding:{stage_id}"],
                "uncertainty": [],
                "open_questions": [],
            }
            for stage_id in contract.stage_ids
        ],
        "domain_details": {field: [f"bounded {field}"] for field in contract.domain_fields},
        "retention": "transient_provider_response_only;validated_against_reviewed_domain_contract",
        "secret_material": "never_returned",
    }


def test_every_domain_has_a_tamper_evident_quality_policy() -> None:
    assert assert_autonomous_domain_quality_policy_coverage() is True
    policies = builtin_autonomous_domain_quality_policies()
    assert len(policies) == 12
    for policy in policies:
        assert len(policy.critical_detail_fields) >= 4
        assert len(policy.safety_detail_fields) >= 2
        assert len(policy.prompt_instructions) >= 4
        assert validate_autonomous_domain_quality_policy(policy) == policy
        reordered = {key: policy.to_dict()[key] for key in reversed(tuple(policy.to_dict()))}
        assert validate_autonomous_domain_quality_policy(reordered) == policy
        assert policy.domain in autonomous_domain_quality_prompt(policy)


def test_quality_policy_produces_perfect_readiness_for_all_domains() -> None:
    for profile in builtin_autonomous_domain_profiles():
        contract = _contract(profile.domain)
        response = validate_autonomous_domain_response(_response(contract), contract)
        report = evaluate_autonomous_domain_response_quality(response, contract)
        assert report.score == 1.0
        assert report.passed is True
        assert report.missing_signals == ()
        assert report.report_digest and len(report.report_digest) == 64


def test_quality_gate_catches_domain_specific_omissions_and_status_drift() -> None:
    contract = _contract("operations")
    response = _response(contract)
    response["domain_details"]["rollback_and_recovery"] = []
    response["stages"][0]["evidence"] = []
    normalized = validate_autonomous_domain_response(response, contract)
    report = evaluate_autonomous_domain_response_quality(normalized, contract)
    assert report.passed is False
    assert "quality_stage_contract_coverage" in report.missing_signals
    assert "quality_safety_control_coverage" in report.missing_signals
    assert any("rollback_and_recovery" in item for item in report.recommendations)

    incoherent = _response(contract)
    incoherent["status"] = "partial"
    report = evaluate_autonomous_domain_response_quality(
        validate_autonomous_domain_response(incoherent, contract), contract
    )
    assert report.signals["quality_status_coherence"] == 0.0
    assert report.passed is False


def test_quality_policy_is_provider_free_and_non_authoritative() -> None:
    policy = autonomous_domain_quality_policy("biomedical")
    assert policy.retention == "policy_metadata_only;does_not_establish_external_truth"
    assert policy.secret_material == "never_returned"
    assert any("medical authorization" in instruction for instruction in policy.prompt_instructions)
    assert AUTONOMOUS_DOMAIN_QUALITY_PASS_THRESHOLD == 0.8
