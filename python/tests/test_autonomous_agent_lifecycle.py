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
    assert report["components"][5]["status"] == "not_attempted"
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
    assert flushed["components"][8]["status"] == "flushed"
    assert flushed["components"][8]["snapshot_digest"] == restored["components"][0]["snapshot_digest"]
    assert "credentials" not in json.dumps(restored)
