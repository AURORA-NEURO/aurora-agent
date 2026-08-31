from __future__ import annotations

import copy

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    LLMRuntime,
    build_autonomous_domain_operating_kit,
    build_autonomous_domain_operating_kits,
    validate_autonomous_domain_operating_kit,
)
from prism_sdk.errors import ArgumentError


def test_builds_complete_operating_kits_for_every_domain() -> None:
    kits = build_autonomous_domain_operating_kits()

    assert tuple(kit.domain for kit in kits) == AUTONOMOUS_DOMAIN_NAMES
    assert len(kits) == 12
    for kit in kits:
        assert kit.status == "complete"
        assert all(kit.coverage.values())
        assert len(kit.stages) >= 4
        assert kit.capability_graph
        assert len(kit.kit_digest) == 64
        assert all(stage.prompt_candidate_ids and stage.selected_prompt_id for stage in kit.stages)
        assert all(stage.tool_names for stage in kit.stages)
        assert all(len(stage.stage_digest) == 64 for stage in kit.stages)
        assert "api_key" not in str(kit.to_dict())


def test_operating_kit_round_trip_and_validation_are_digest_bound() -> None:
    kit = build_autonomous_domain_operating_kit("operations")
    projection = kit.to_dict()

    assert validate_autonomous_domain_operating_kit(projection).to_dict() == projection

    tampered_stage = copy.deepcopy(projection)
    tampered_stage["stages"][0]["objective"] = "unreviewed objective"
    with pytest.raises(ArgumentError, match="digest|stale or tampered"):
        validate_autonomous_domain_operating_kit(tampered_stage)

    tampered_kit = copy.deepcopy(projection)
    tampered_kit["next_actions"].append("tampered handoff")
    with pytest.raises(ArgumentError, match="digest|stale or tampered"):
        validate_autonomous_domain_operating_kit(tampered_kit)


def test_brain_facade_reads_operating_contracts_without_provider_activity() -> None:
    agent = AutonomousAgent(None, LLMRuntime())

    kit = agent.domain_operating_kit("evaluation")
    selected = agent.domain_operating_kits(("coding", "evaluation"))

    assert kit.domain == "evaluation"
    assert tuple(item.domain for item in selected) == ("coding", "evaluation")
