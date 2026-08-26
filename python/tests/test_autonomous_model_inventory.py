from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainRunError,
    AutonomousModelInventoryError,
    AutonomousModelInventoryStore,
    LLMRuntime,
    ModelCatalogue,
    ProviderError,
)


def _runtime(rows, *, failing: bool = False) -> LLMRuntime:
    runtime = LLMRuntime()

    def discover():
        if failing:
            raise ProviderError("provider inventory is unavailable")
        return {"data": list(rows)}

    runtime.register_in_memory_provider(
        "offline",
        lambda request: "offline response",
        model_discovery_handler=discover,
    )
    return runtime


def _prior(arm: str) -> dict[str, object]:
    return {
        arm: {
            "quality": 0.82,
            "latency_ms": 120,
            "cost_per_million_tokens": 2,
            "reliability": 0.91,
        }
    }


def test_inventory_refreshes_models_and_reports_all_domain_coverage(tmp_path):
    required_capabilities: list[str] = []
    rows = [
        {
            "id": "offline-model",
            "context_length": 32_000,
            "max_output_tokens": 2_000,
            "capabilities": required_capabilities,
        }
    ]
    runtime = _runtime(rows)
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    required_capabilities.extend(
        sorted(
            {
                capability
                for pack in agent.domain_packs()
                for capability in pack["model_capabilities"]
            }
            | {"structured_output", "tool_calling"}
        )
    )
    store = AutonomousModelInventoryStore(tmp_path / "inventory.json")

    snapshot = agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/offline-model"),
        snapshot_store=store,
        refresh_id="inventory-test-1",
    )

    assert snapshot["status"] == "completed"
    assert snapshot["providers"][0]["status"] == "refreshed"
    assert snapshot["providers"][0]["model_ids"] == ["offline-model"]
    assert set(row["domain"] for row in snapshot["coverage"]) == set(AUTONOMOUS_DOMAINS)
    assert all(row["compatible_count"] == 1 for row in snapshot["coverage"])
    assert agent.models()[0]["provider"] == "offline"
    restored = store.load()
    assert restored is not None
    assert restored.digest == snapshot["snapshot_digest"]
    restored_catalogue = store.load_catalogue()
    assert restored_catalogue is not None
    assert restored_catalogue.candidates()[0]["model"] == "offline-model"
    assert "offline response" not in json.dumps(snapshot)


def test_inventory_retires_stale_models_only_after_successful_authoritative_refresh():
    rows = [{"id": "old-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}]
    runtime = _runtime(rows)
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    first = agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/old-model"),
        domain_requirements={"coding": ("reasoning",)},
        refresh_id="inventory-old",
    )
    assert first["providers"][0]["removed_model_ids"] == []

    rows[:] = [{"id": "new-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}]
    second = agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/new-model"),
        domain_requirements={"coding": ("reasoning",)},
        refresh_id="inventory-new",
    )
    assert second["providers"][0]["removed_model_ids"] == ["offline/old-model"]
    assert [row["model"] for row in agent.models()] == ["new-model"]


def test_agent_inventory_restore_retains_cas_fence_and_rolls_back_stale_refresh(tmp_path):
    rows = [{"id": "old-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}]
    runtime = _runtime(rows)
    store = AutonomousModelInventoryStore(tmp_path / "inventory-agent-cas.json")
    primary = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    first = primary.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/old-model"),
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="inventory-agent-old",
    )

    restarted = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    restored = restarted.restore_model_inventory(store)
    assert restored is not None
    assert restored["snapshot_digest"] == first["snapshot_digest"]
    assert [row["model"] for row in restarted.models()] == ["old-model"]

    rows[:] = [{"id": "new-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}]
    second = primary.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/new-model"),
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="inventory-agent-new",
    )
    assert second["snapshot_digest"] != first["snapshot_digest"]

    with pytest.raises(BrainRunError, match="model inventory refresh failed"):
        restarted.refresh_model_inventory(
            providers=("offline",),
            priors=_prior("offline/new-model"),
            domain_requirements={"coding": ("reasoning",)},
            snapshot_store=store,
            refresh_id="inventory-agent-stale",
        )
    assert [row["model"] for row in restarted.models()] == ["old-model"]


def test_agent_inventory_restore_rejects_snapshot_without_bound_catalogue(tmp_path):
    runtime = _runtime([{"id": "offline-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}])
    source = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    snapshot = source.model_inventory.refresh(
        providers=("offline",),
        priors=_prior("offline/offline-model"),
        domain_requirements={"coding": ("reasoning",)},
        refresh_id="inventory-unbound-catalogue",
    )
    store = AutonomousModelInventoryStore(tmp_path / "inventory-unbound-catalogue.json")
    store.save(snapshot)

    with pytest.raises(BrainRunError, match="model inventory restore failed"):
        AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue()).restore_model_inventory(store)


def test_inventory_prior_factory_derives_explicit_metadata_without_second_discovery():
    rows = [
        {
            "id": "factory-model",
            "context_length": 24_000,
            "max_output_tokens": 1_500,
            "capabilities": ["tool_calling"],
        }
    ]
    runtime = _runtime(rows)
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    seen: list[str] = []

    def prior_factory(descriptor):
        seen.append(descriptor.arm_id)
        return {
            "quality": 0.73,
            "latency_ms": 240,
            "cost_per_million_tokens": 3,
            "reliability": 0.88,
            "capabilities": ["reasoning"],
        }

    snapshot = agent.refresh_model_inventory(
        providers=("offline",),
        prior_factory=prior_factory,
        domain_requirements={"coding": ("reasoning", "tool_calling")},
        refresh_id="inventory-factory",
    )

    assert snapshot["status"] == "completed"
    assert seen == ["offline/factory-model"]
    candidate = agent.models()[0]
    assert candidate["context_window_tokens"] == 24_000
    assert candidate["max_output_tokens"] == 1_500
    assert candidate["quality"] == 0.73
    assert set(candidate["capabilities"]) == {"reasoning", "tool_calling"}
    assert snapshot["coverage"][0]["compatible_count"] == 1


def test_failed_provider_does_not_retire_other_provider_arms_or_leak_errors():
    rows = [{"id": "good-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}]
    runtime = _runtime(rows)
    runtime.register_in_memory_provider(
        "broken",
        lambda request: "unused",
        model_discovery_handler=lambda: (_ for _ in ()).throw(ProviderError("do not serialize this")),
    )
    runtime.register_in_memory_provider(
        "good",
        lambda request: "good response",
        model_discovery_handler=lambda: {"data": list(rows)},
    )
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    snapshot = agent.refresh_model_inventory(
        providers=("good", "broken"),
        priors=_prior("good/good-model"),
        domain_requirements={"coding": ("reasoning",)},
        refresh_id="inventory-partial",
    )

    assert snapshot["status"] == "partial"
    assert {row["status"] for row in snapshot["providers"]} == {"refreshed", "provider_failed"}
    assert [row["model"] for row in agent.models()] == ["good-model"]
    assert "do not serialize this" not in json.dumps(snapshot)


def test_missing_prior_fails_closed_without_mutating_catalogue():
    runtime = _runtime([{"id": "unpriced", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}])
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())

    snapshot = agent.refresh_model_inventory(
        providers=("offline",),
        priors={},
        domain_requirements={"coding": ("reasoning",)},
        refresh_id="inventory-missing-prior",
    )

    assert snapshot["status"] == "failed"
    assert snapshot["providers"][0]["error_class"] == "provider"
    assert agent.models() == []


def test_inventory_store_rejects_tampered_snapshot(tmp_path):
    runtime = _runtime([{"id": "offline-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}])
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    store = AutonomousModelInventoryStore(tmp_path / "inventory.json")
    snapshot = agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/offline-model"),
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="inventory-tamper",
    )
    payload = json.loads(store.path.read_text(encoding="utf-8"))
    payload["snapshot"]["catalogue_digest"] = "0" * 64
    store.path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(AutonomousModelInventoryError):
        store.load()
    assert snapshot["snapshot_digest"] != "0" * 64


def test_inventory_store_rejects_tampered_rehydrated_catalogue(tmp_path):
    runtime = _runtime([{"id": "offline-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}])
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    store = AutonomousModelInventoryStore(tmp_path / "inventory.json")
    agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/offline-model"),
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="inventory-catalogue-tamper",
    )
    payload = json.loads(store.path.read_text(encoding="utf-8"))
    payload["catalogue"]["candidates"][0]["quality"] = 0.01
    store.path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(AutonomousModelInventoryError):
        store.load_catalogue()


def test_inventory_store_fences_stale_refreshes_and_requires_canonical_json(tmp_path):
    runtime = _runtime([{"id": "offline-model", "context_length": 16_000, "max_output_tokens": 1_000, "capabilities": ["reasoning"]}])
    agent = AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue())
    store = AutonomousModelInventoryStore(tmp_path / "inventory-cas.json")
    agent.refresh_model_inventory(
        providers=("offline",),
        priors=_prior("offline/offline-model"),
        domain_requirements={"coding": ("reasoning",)},
        snapshot_store=store,
        refresh_id="inventory-cas",
    )
    snapshot = store.load()
    assert snapshot is not None
    catalogue = store.load_catalogue()
    assert catalogue is not None
    assert not store.save_if_unchanged(snapshot, "0" * 64, catalogue=catalogue)
    assert store.save_if_unchanged(snapshot, snapshot.digest, catalogue=catalogue)
    store.path.write_text(json.dumps(json.loads(store.path.read_text(encoding="utf-8")), indent=2), encoding="utf-8")
    with pytest.raises(AutonomousModelInventoryError, match="canonical"):
        store.load()
