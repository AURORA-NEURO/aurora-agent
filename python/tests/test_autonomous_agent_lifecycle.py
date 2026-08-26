from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER,
    AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER,
    AutonomousAgentPersistenceLifecycleCoordinator,
    AutonomousAgentPersistenceLifecycleError,
    AutonomousAgent,
    AutonomousCapabilityActivationStore,
    AutonomousSelectionPromotionLifecycle,
    AutonomousSelectionPromotionLifecycleStore,
    AutonomousModelInventoryStore,
    AutonomousCapabilityJournalPersistenceCoordinator,
    InMemoryAutonomousCapabilityJournalStore,
    AutonomousDecisionCycle,
    AutonomousDecisionCyclePersistenceCoordinator,
    InMemoryAutonomousDecisionCycleStateStore,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPersistenceCoordinator,
    AutonomousExecutionPolicy,
    LLMRuntime,
    ModelCatalogue,
)


class _LifecycleAgent:
    def __init__(self, calls: list[str], *, fail: str | None = None) -> None:
        self.calls = calls
        self.fail = fail
        self.selection_promotion = object()
        for component in (
            "runtime_health",
            "health",
            "evaluator_calibration",
            "memory",
            "learning",
            "prompt_learning",
            "capability_journal",
            "decision_cycle",
            "execution",
        ):
            setattr(self, f"{component}_persistence", object())

    @staticmethod
    def _value(component: str, operation: str) -> dict[str, object]:
        return {
            "schema": f"test/{component}/{operation}",
            "snapshot_digest": "a" * 64,
            "state_digest": "b" * 64,
            "generation": 3,
        }

    def restore_model_inventory(self, store: object) -> dict[str, object]:
        self.calls.append("restore:model_inventory")
        return self._value("model_inventory", "restore")

    def flush_model_inventory(self, store: object) -> dict[str, object]:
        self.calls.append("flush:model_inventory")
        return self._value("model_inventory", "flush")

    def restore_capability_journal_persistence(self) -> dict[str, object]:
        self.calls.append("restore:capability_journal")
        return self._value("capability_journal", "restore")

    def flush_capability_journal_persistence(self) -> dict[str, object]:
        self.calls.append("flush:capability_journal")
        return self._value("capability_journal", "flush")

    def restore_decision_cycle_persistence(self) -> dict[str, object]:
        self.calls.append("restore:decision_cycle")
        return self._value("decision_cycle", "restore")

    def flush_decision_cycle_persistence(self) -> dict[str, object]:
        self.calls.append("flush:decision_cycle")
        return self._value("decision_cycle", "flush")

    def restore_execution_persistence(self) -> dict[str, object]:
        self.calls.append("restore:execution")
        return self._value("execution", "restore")

    def flush_execution_persistence(self) -> dict[str, object]:
        self.calls.append("flush:execution")
        return self._value("execution", "flush")

    def __getattr__(self, name: str):
        if name.startswith("restore_") or name.startswith("flush_") or name.startswith("save_"):
            operation, component = name.split("_", 1)
            if operation == "save":
                operation = "flush"

            def invoke(*args: object) -> dict[str, object]:
                self.calls.append(f"{operation}:{component}")
                if component == self.fail:
                    raise ValueError("private task/prompt/provider payload must not escape")
                return self._value(component, operation)

            return invoke
        raise AttributeError(name)


def test_lifecycle_restores_and_flushes_in_explicit_dependency_order() -> None:
    calls: list[str] = []
    agent = _LifecycleAgent(calls)
    coordinator = AutonomousAgentPersistenceLifecycleCoordinator(
        agent,
        model_inventory_store=object(),
        activation_store=object(),
        selection_promotion_store=object(),
        capability_journal_persistence=object(),
        decision_cycle_persistence=object(),
        execution_persistence=object(),
        require_all=True,
    )

    restored = coordinator.restore().to_dict()
    assert restored["status"] == "completed"
    assert restored["ordered_component_ids"] == list(AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER)
    assert calls == [f"restore:{component}" for component in AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER]
    assert restored["lifecycle_digest"]

    calls.clear()
    flushed = coordinator.flush().to_dict()
    assert flushed["status"] == "completed"
    assert flushed["ordered_component_ids"] == list(AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER)
    assert calls == [f"flush:{component}" for component in AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER]
    assert flushed["atomicity"] == "per_component_cas_only;cross_store_atomicity_caller_owned"


def test_lifecycle_strict_failure_keeps_redacted_report_and_not_attempted_rows() -> None:
    calls: list[str] = []
    agent = _LifecycleAgent(calls, fail="health")
    coordinator = AutonomousAgentPersistenceLifecycleCoordinator(
        agent,
        model_inventory_store=object(),
        activation_store=object(),
        selection_promotion_store=object(),
        require_all=True,
    )

    with pytest.raises(AutonomousAgentPersistenceLifecycleError) as raised:
        coordinator.restore(strict=True)
    report = raised.value.report.to_dict()
    assert report["failed_component_id"] == "health"
    assert report["components"][2]["status"] == "failed"
    assert report["components"][2]["error_class"] == "ValueError"
    assert report["components"][6]["status"] == "not_attempted"
    assert "private task/prompt/provider payload" not in json.dumps(report)
    assert coordinator.last_report is raised.value.report


def test_lifecycle_non_strict_mode_surfaces_unconfigured_components() -> None:
    calls: list[str] = []

    class PartialAgent(_LifecycleAgent):
        def __init__(self) -> None:
            super().__init__(calls)
            self.memory_persistence = None

    report = AutonomousAgentPersistenceLifecycleCoordinator(
        PartialAgent(),
        model_inventory_store=object(),
        require_all=False,
    ).restore(strict=False).to_dict()
    assert report["status"] == "partial"
    assert "memory" in report["unconfigured_component_ids"]
    assert report["next_action"] == "bind_unconfigured_persistence_or_accept_partial_lifecycle"


def _runtime() -> LLMRuntime:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "offline",
        lambda request: "unused",
        model_discovery_handler=lambda: {
            "data": [{
                "id": "lifecycle-model",
                "context_length": 16_000,
                "max_output_tokens": 1_000,
                "capabilities": ["reasoning"],
            }]
        },
    )
    return runtime


def test_agent_persisted_state_composes_inventory_restore_and_flush_without_provider_calls(tmp_path) -> None:
    runtime = _runtime()
    store = AutonomousModelInventoryStore(tmp_path / "lifecycle-inventory.json")
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    agent.refresh_model_inventory(
        providers=("offline",),
        priors={"offline/lifecycle-model": {"quality": 0.8, "latency_ms": 20, "cost_per_million_tokens": 0, "reliability": 0.9}},
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="lifecycle-inventory",
    )
    activation_store = AutonomousCapabilityActivationStore(tmp_path / "lifecycle-activation.json")
    selection_store = AutonomousSelectionPromotionLifecycleStore()
    agent.selection_promotion = AutonomousSelectionPromotionLifecycle()
    agent.save_activation(activation_store)
    agent.save_selection_promotion(selection_store)

    restarted = AutonomousAgent(
        object(),
        runtime,
        model_catalogue=ModelCatalogue(),
        selection_promotion=AutonomousSelectionPromotionLifecycle(),
    )
    restored = restarted.restore_persisted_state(
        model_inventory_store=store,
        activation_store=activation_store,
        selection_promotion_store=selection_store,
        strict=False,
    )
    assert restored["components"][0]["status"] == "restored"
    assert restored["components"][3]["status"] == "restored"
    assert restored["components"][4]["status"] == "restored"
    assert restored["components"][0]["snapshot_digest"]
    assert [row["model"] for row in restarted.models()] == ["lifecycle-model"]

    flushed = restarted.flush_persisted_state(
        model_inventory_store=store,
        activation_store=activation_store,
        selection_promotion_store=selection_store,
        strict=False,
    )
    assert flushed["components"][11]["status"] == "flushed"
    assert flushed["components"][11]["snapshot_digest"] == restored["components"][0]["snapshot_digest"]
    assert "credentials" not in json.dumps(restored)


class _MetadataSnapshotStore:
    def __init__(self) -> None:
        self.value: object | None = None

    def read(self) -> object | None:
        return self.value

    def write(self, value: object) -> None:
        self.value = value.to_dict() if hasattr(value, "to_dict") else dict(value) if isinstance(value, dict) else value


def test_agent_lifecycle_restores_capability_barrier_before_execution_checkpoint(tmp_path) -> None:
    runtime = _runtime()
    capability_store = InMemoryAutonomousCapabilityJournalStore()
    capability_persistence = AutonomousCapabilityJournalPersistenceCoordinator(
        capability_store,
        _MetadataSnapshotStore(),
    )
    execution_journal = AutonomousExecutionJournal(tmp_path / "source-execution.jsonl")
    execution_persistence = AutonomousExecutionPersistenceCoordinator(
        execution_journal,
        _MetadataSnapshotStore(),
    )
    decision_store = InMemoryAutonomousDecisionCycleStateStore()
    decision_persistence = AutonomousDecisionCyclePersistenceCoordinator(
        decision_store,
        _MetadataSnapshotStore(),
    )
    AutonomousDecisionCycle(
        decision_store,
        cycle_id="lifecycle-decision",
        task="restart-safe lifecycle decision",
        mode="single_domain",
    )
    source = AutonomousAgent(
        object(),
        runtime,
        capability_journal=capability_store,
        capability_journal_persistence=capability_persistence,
        execution_journal=execution_journal,
        decision_cycle_persistence=decision_persistence,
        execution_persistence=execution_persistence,
    )
    controller = AutonomousExecutionController(
        execution_id="lifecycle-recovery",
        domain="coding",
        capability="planning",
        risk_class="read_only",
        policy=AutonomousExecutionPolicy(max_steps=8, max_tool_calls=2),
        journal=execution_journal,
    )
    controller.checkpoint(status="paused", reason="process_restart")
    flushed = source.flush_persisted_state(strict=False)
    assert flushed["ordered_component_ids"][:3] == ["execution", "decision_cycle", "capability_journal"]
    assert flushed["components"][0]["status"] == "flushed"
    assert flushed["components"][1]["status"] == "flushed"
    assert flushed["components"][1]["snapshot_digest"]

    restored_capability_store = InMemoryAutonomousCapabilityJournalStore()
    restored_capability_persistence = AutonomousCapabilityJournalPersistenceCoordinator(
        restored_capability_store,
        capability_persistence.persistence,
    )
    restored_execution_journal = AutonomousExecutionJournal(tmp_path / "restored-execution.jsonl")
    restored_execution_persistence = AutonomousExecutionPersistenceCoordinator(
        restored_execution_journal,
        execution_persistence.persistence,
    )
    restored_decision_store = InMemoryAutonomousDecisionCycleStateStore()
    restored_decision_persistence = AutonomousDecisionCyclePersistenceCoordinator(
        restored_decision_store,
        decision_persistence.persistence,
    )
    restarted = AutonomousAgent(
        object(),
        runtime,
        capability_journal=restored_capability_store,
        capability_journal_persistence=restored_capability_persistence,
        execution_journal=restored_execution_journal,
        decision_cycle_persistence=restored_decision_persistence,
        execution_persistence=restored_execution_persistence,
    )
    restored = restarted.restore_persisted_state(strict=False)
    assert restored["components"][-3]["status"] == "restored"
    assert restored["components"][-2]["status"] == "restored"
    assert restored["components"][-1]["status"] == "restored"
    assert restored["components"][-2]["snapshot_digest"]
    assert restored["components"][-3]["generation"] == 1
    assert restarted.execution_state("lifecycle-recovery")["status"] == "paused"
    assert restored_decision_store.load("lifecycle-decision") is not None
    assert "rows" not in json.dumps(restored)
