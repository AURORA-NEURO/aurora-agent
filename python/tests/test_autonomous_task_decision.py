from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    autonomous_domain_policy,
    autonomous_domain_task_lens,
    content_digest,
    infer_autonomous_task_decision,
    infer_autonomous_task_intent,
)
from prism_sdk.errors import ArgumentError


def _decision(task: str, domain: str):
    intent = infer_autonomous_task_intent(
        task=task,
        task_digest=content_digest({"task": task}),
        domain=domain,
        capability="review",
        risk_class="read_only",
        workflow_id=f"{domain}_workflow",
        lens=autonomous_domain_task_lens(domain),
        desired_outputs=("review decision",),
    )
    return infer_autonomous_task_decision(
        intent=intent,
        lens=autonomous_domain_task_lens(domain),
        policy=autonomous_domain_policy(domain),
        required_model_capabilities=("reasoning", "structured_output"),
    )


def test_task_decision_is_digest_bound_and_blocks_forbidden_biomedical_effects() -> None:
    task = "deploy the biomedical report and verify safety"
    intent = infer_autonomous_task_intent(
        task=task,
        task_digest=content_digest({"task": task}),
        domain="biomedical",
        capability="biomedical_analysis",
        risk_class="clinical_review",
        workflow_id="biomedical_review",
        lens=autonomous_domain_task_lens("biomedical"),
        desired_outputs=("safety boundary",),
    )
    decision = infer_autonomous_task_decision(
        intent=intent,
        lens=autonomous_domain_task_lens("biomedical"),
        policy=autonomous_domain_policy("biomedical"),
        required_model_capabilities=("reasoning", "biomedical", "structured_output"),
    )
    public = decision.to_dict()
    assert decision.decision_digest == "29a60c4c19879b835edb25c6b20ce6e0c9a12b9cfa479a24d1420714f039e848"
    assert decision.posture == "blocked"
    assert decision.recommended_path == "evidence_first"
    assert "requested_effect_forbidden_by_domain_policy" in decision.blocking_reasons
    assert "evidence_dispatch" in decision.approval_requirements
    assert task not in json.dumps(public)
    assert public["authorization"] == "guidance_only;provider_source_tool_and_effect_authority_remain_separate"


def test_task_decision_covers_all_domains_and_rejects_invalid_inputs() -> None:
    for domain in AUTONOMOUS_DOMAINS:
        decision = _decision(f"analyze the {domain} workflow", domain)
        assert decision.domain == domain
        assert decision.decision_digest and len(decision.decision_digest) == 64
        assert decision.approval_requirements
        assert decision.next_actions

    with pytest.raises(ArgumentError):
        intent = infer_autonomous_task_intent(
            task="analyze the data workflow",
            task_digest=content_digest({"task": "analyze the data workflow"}),
            domain="data",
            capability="review",
            risk_class="read_only",
            workflow_id="data_workflow",
            lens=autonomous_domain_task_lens("data"),
        )
        infer_autonomous_task_decision(
            intent=intent,
            lens=autonomous_domain_task_lens("data"),
            policy=autonomous_domain_policy("data"),
            required_model_capabilities=(),
        )
