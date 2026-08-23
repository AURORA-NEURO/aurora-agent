from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES,
    autonomous_domain_task_lens,
    content_digest,
    infer_autonomous_task_intent,
)
from prism_sdk.errors import ArgumentError


def test_task_intent_is_bounded_domain_specific_and_cross_runtime_canonical() -> None:
    task = "deploy the biomedical report and verify safety"
    intent = infer_autonomous_task_intent(
        task=task,
        task_digest=content_digest({"task": task}),
        domain="biomedical",
        capability="biomedical_analysis",
        risk_class="clinical_review",
        workflow_id="biomedical_review",
        lens=autonomous_domain_task_lens("biomedical"),
        constraints=("qualified review",),
        desired_outputs=("safety boundary",),
    )
    public = intent.to_dict()
    assert intent.intent_digest == "e984ee4cabfaa0a2463ead4c6d4042a85ba5da9d5ad28283f0b6257366f6508d"
    assert intent.action_mode == "evaluate"
    assert intent.requested_effect == "external_effect"
    assert intent.evidence_mode == "grounding_and_safety_evidence"
    assert "effect_requires_explicit_approval" in intent.ambiguity_flags
    assert "human_review_boundary" in intent.risk_signals
    assert public["authorization"] == "classification_only;no_provider_tool_or_effect_authority"
    assert task not in json.dumps(public)
    assert public["secret_material"] == "never_returned"

    for domain in AUTONOMOUS_DOMAINS:
        domain_intent = infer_autonomous_task_intent(
            task=f"review the {domain} workflow and report verification gaps",
            task_digest=content_digest({"task": f"review the {domain} workflow and report verification gaps"}),
            domain=domain,
            capability="reasoning",
            risk_class="read_only",
            workflow_id=f"{domain}_workflow",
            lens=autonomous_domain_task_lens(domain),
        )
        assert domain_intent.domain == domain
        assert domain_intent.evidence_mode in AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES
        assert domain_intent.intent_digest and len(domain_intent.intent_digest) == 64
        assert domain_intent.planning_signals
        assert domain_intent.success_signals


def test_task_intent_rejects_digest_drift_and_lens_domain_mismatch() -> None:
    task = "analyze the dataset lineage"
    lens = autonomous_domain_task_lens("data")
    with pytest.raises(ArgumentError):
        infer_autonomous_task_intent(
            task=task,
            task_digest="0" * 64,
            domain="data",
            capability="data_analysis",
            risk_class="read_only",
            workflow_id="data_analysis",
            lens=lens,
        )

    with pytest.raises(ArgumentError):
        infer_autonomous_task_intent(
            task=task,
            task_digest=content_digest({"task": task}),
            domain="science",
            capability="scientific_reasoning",
            risk_class="read_only",
            workflow_id="science_review",
            lens=lens,
        )


def test_task_intent_rejects_malformed_or_duplicate_input_items() -> None:
    task = "analyze the dataset"
    lens = autonomous_domain_task_lens("data")
    kwargs = {
        "task": task,
        "task_digest": content_digest({"task": task}),
        "domain": "data",
        "capability": "data_analysis",
        "risk_class": "read_only",
        "workflow_id": "data_analysis",
        "lens": lens,
    }
    with pytest.raises(ArgumentError):
        infer_autonomous_task_intent(**kwargs, constraints=("schema", "schema"))
    with pytest.raises(ArgumentError):
        infer_autonomous_task_intent(**kwargs, desired_outputs=("",))
