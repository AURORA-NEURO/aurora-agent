import json

import pytest

from prism_sdk.autonomous_capability_routing import (
    AUTONOMOUS_CAPABILITY_ROUTE_REASONS,
    autonomous_capability_vocabulary,
    route_autonomous_capability,
    validate_autonomous_capability_route,
)
from prism_sdk.autonomy import (
    AUTONOMOUS_DOMAINS,
    AutonomousTaskOrchestrator,
    _memory_selection_context,
)
from prism_sdk.brain import AutonomousBrain
from prism_sdk.llm_runtime import LLMRuntime


EXAMPLES = {
    "coding": ("debug a failing stack trace", "debugging"),
    "browser": ("compare sources and verify sources", "source_comparison"),
    "data": ("trace data lineage and provenance", "lineage"),
    "science": ("review the literature and references", "literature"),
    "biomedical": ("require human review by a clinician", "human_review"),
    "neuroscience": ("interpret an EEG neural signal", "signal_interpretation"),
    "operations": ("rollback the production service", "rollback"),
    "enterprise": ("map the governance policy and owner", "governance"),
    "multi_agent": ("resolve the agent conflict and disagreement", "conflict_resolution"),
    "multimodal": ("align modalities for cross modal fusion", "cross_modal_alignment"),
    "cross_domain": ("synthesize the specialist findings", "synthesis"),
    "evaluation": ("replay the deterministic evaluation trace", "replay"),
}

PARITY_DIGESTS = {
    "coding": "0a4b70be55be8d9e92e9f8583b064e0eef0d04c820d6c9dd2b9912578cd15ad3",
    "operations": "63bdb39cae43015b485160f290189bc2a757c6627d64513c6c6004d281109633",
}


def test_provider_free_capability_routing_selects_all_domains() -> None:
    for domain in AUTONOMOUS_DOMAINS:
        task, expected = EXAMPLES[domain]
        route = route_autonomous_capability(task, domain)
        assert route.domain == domain
        assert route.selected_capability == expected
        assert route.abstained is False
        assert route.reason == "selected"
        assert len(route.route_digest) == 64
        assert expected in autonomous_capability_vocabulary(domain)
        if domain in PARITY_DIGESTS:
            assert route.route_digest == PARITY_DIGESTS[domain]
        assert validate_autonomous_capability_route(task, route) == route


def test_capability_routing_abstains_and_supports_explicit_overrides() -> None:
    unknown = route_autonomous_capability("zzzz qqqq", "coding")
    assert unknown.abstained is True
    assert unknown.reason == "no_matching_capability"
    assert unknown.selected_capability is None

    ambiguous = route_autonomous_capability("schema quality", "data", min_margin=0.5)
    assert ambiguous.abstained is True
    assert ambiguous.reason == "insufficient_margin"

    explicit = route_autonomous_capability("perform the bounded task", "coding", explicit_capability="custom_review")
    assert explicit.selected_capability == "custom_review"
    assert explicit.reason == "explicit_capability"
    with pytest.raises(Exception, match="task digest"):
        validate_autonomous_capability_route("a different task", explicit)
    tampered = explicit.to_dict()
    tampered["confidence"] = 0.5
    with pytest.raises(Exception, match="digest"):
        validate_autonomous_capability_route("perform the bounded task", tampered)
    assert set(AUTONOMOUS_CAPABILITY_ROUTE_REASONS) >= {"selected", "explicit_capability"}


def test_automatic_python_blueprints_use_selected_capability() -> None:
    blueprint = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime())).prepare(
        task="debug a failing stack trace",
        domain="coding",
    )
    assert blueprint.capability_route is not None
    assert blueprint.capability_route.selected_capability == "debugging"
    assert blueprint.spec.capability == "debugging"
    assert blueprint.selection_context["capability"] == "debugging"
    assert blueprint.task_intent is not None
    assert blueprint.task_intent.capability == "debugging"


def test_automatic_python_cross_domain_children_keep_capability_identity() -> None:
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    automatic = orchestrator.prepare_auto(
        task="coordinate coding and biomedical evidence across disciplines",
        hints=("coding", "biomedical"),
        max_domains=2,
    )
    assert automatic.cross_domain_blueprint is not None
    children = automatic.cross_domain_blueprint.child_blueprints
    assert {child.profile.domain for child in children} == {"coding", "biomedical"}
    for child in children:
        assert child.capability_route is not None
        expected = child.capability_route.selected_capability or child.profile.default_capability
        assert child.spec.capability == expected
        assert child.selection_context["capability"] == expected
        assert child.task_intent is not None
        assert child.task_intent.capability == expected


def test_memory_projection_keeps_route_identity_inside_its_bounded_envelope() -> None:
    blueprint = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime())).prepare(
        task="debug a failing stack trace",
        domain="coding",
    )
    projected = _memory_selection_context(blueprint)
    assert len(projected) <= 32
    assert projected["capability"] == "debugging"
    assert projected["capability_route_digest"] == blueprint.capability_route.route_digest
    assert "domain_capabilities" not in projected
