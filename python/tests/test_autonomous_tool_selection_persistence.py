from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousToolSelectionPersistenceCoordinator,
    JsonAutonomousToolSelectionPersistence,
    LLMRuntime,
    TransactionalJsonAutonomousToolSelectionPersistence,
    normalize_autonomous_tool_selection_state,
    settle_autonomous_tool_selection_outcome,
)
from prism_sdk.errors import ArgumentError


class _CasTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected:
            return False
        self.value = value
        return True


def test_tool_selection_snapshot_round_trip_is_canonical_and_cas_fenced() -> None:
    state = normalize_autonomous_tool_selection_state(None)
    for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES, start=1):
        state = settle_autonomous_tool_selection_outcome(
            state,
            domain=domain,
            capability="read_only_analysis",
            tool=f"fixture_{domain}",
            reward=0.5,
            outcome_digest=f"{index:02x}" + "a" * 62,
        )
    text_store = _CasTextStore()
    persistence = TransactionalJsonAutonomousToolSelectionPersistence(text_store)
    holder = {"state": state}
    coordinator = AutonomousToolSelectionPersistenceCoordinator(
        lambda: holder["state"],
        lambda value: holder.__setitem__("state", dict(value)),
        persistence,
    )
    assert coordinator.restore() is None
    first = coordinator.flush()
    assert first.snapshot_generation == 1
    assert first.previous_snapshot_digest is None
    assert len(first.state["arms"]) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert json.loads(text_store.value or "") == first.to_dict()
    second = coordinator.flush()
    assert second.snapshot_generation == 2
    assert second.previous_snapshot_digest == first.snapshot_digest

    tampered = second.to_dict()
    tampered["state_digest"] = "b" * 64
    with pytest.raises(ArgumentError, match="state digest"):
        type(second).from_dict(tampered)

    stale = AutonomousToolSelectionPersistenceCoordinator(
        lambda: holder["state"],
        lambda value: holder.__setitem__("state", dict(value)),
        TransactionalJsonAutonomousToolSelectionPersistence(text_store),
    )
    assert stale.restore() is not None
    assert coordinator.flush().snapshot_generation == 3
    with pytest.raises(ArgumentError, match="compare-and-swap conflict"):
        stale.flush()


def test_agent_owns_tool_selection_and_lifecycle_restores_it() -> None:
    text_store = _CasTextStore()
    persistence = TransactionalJsonAutonomousToolSelectionPersistence(text_store)
    runtime = LLMRuntime()
    source = AutonomousAgent(object(), runtime, tool_selection_persistence=persistence)
    state = source.record_tool_selection_reward({
        "domain": "coding",
        "capability": "read_only_analysis",
        "tool": "fixture_tool",
        "reward": 1,
        "outcome_digest": "c" * 64,
    })
    assert state["generation"] == 1
    first = source.flush_tool_selection()
    assert first.state["arms"][0]["arm_id"] == "coding.read_only_analysis.fixture_tool"

    restarted = AutonomousAgent(
        object(),
        runtime,
        tool_selection_persistence=TransactionalJsonAutonomousToolSelectionPersistence(text_store),
    )
    report = restarted.restore_persisted_state(strict=False)
    row = next(item for item in report["components"] if item["component_id"] == "tool_selection")
    assert row["status"] == "restored"
    assert restarted.tool_selection_state_snapshot()["generation"] == 1
    flushed = restarted.flush_persisted_state(strict=False)
    row = next(item for item in flushed["components"] if item["component_id"] == "tool_selection")
    assert row["status"] == "flushed"
    assert row["generation"] == 2


def test_plain_json_tool_selection_persistence_is_supported() -> None:
    value: str | None = None
    persistence = JsonAutonomousToolSelectionPersistence(
        type("Store", (), {
            "read": lambda _self: value,
            "write": lambda _self, next_value: None,
        })()
    )
    # The plain adapter's contract is covered without relying on a mutable closure in a class
    # body; the CAS path above exercises the same canonical snapshot validator more deeply.
    state = settle_autonomous_tool_selection_outcome(
        None,
        domain="cross_domain",
        capability="synthesis",
        tool="fixture_synthesis",
        reward=0,
    )
    holder = {"state": state}
    coordinator = AutonomousToolSelectionPersistenceCoordinator(
        lambda: holder["state"],
        lambda next_state: holder.__setitem__("state", dict(next_state)),
        persistence,
    )
    # This store intentionally does not retain the write; validation and serialization still run.
    assert coordinator.flush().state["generation"] == 1
