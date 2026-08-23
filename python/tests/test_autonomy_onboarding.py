from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_ACTIVATION_SCHEMA,
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousActivationError,
    AutonomousCapabilityActivation,
    AutonomousCapabilityActivationStore,
    ToolCatalogue,
    canonical_bytes,
    plan_mcp_catalogue_bindings,
)


def _plan(*definitions: dict[str, object]) -> dict[str, object]:
    return plan_mcp_catalogue_bindings(ToolCatalogue.from_definitions(list(definitions)))


def _provider_status() -> dict[str, object]:
    return {
        "provider": "openai",
        "provider_registered": True,
        "requires_credential": True,
        "ready": True,
        "credential": {
            "configured": True,
            "credential_count": 1,
            "credentials": [
                {
                    "credential_id": "opaque-id-must-not-persist",
                    "secret_value": "provider-secret-must-not-persist",
                }
            ],
        },
    }


def test_activation_covers_all_domains_and_never_persists_credential_metadata() -> None:
    activation = AutonomousCapabilityActivation(activation_id="activation-test", clock=lambda: 10.0)
    activation.record_provider_statuses([_provider_status()])
    plan = _plan(
        {
            "name": "repository_catalog",
            "description": "Read bounded repository metadata.",
            "inputSchema": {"type": "object"},
        },
        {
            "name": "tabular_ingest",
            "description": "Ingest a bounded table.",
            "inputSchema": {"type": "object"},
        },
        {
            "name": "unclassified_workspace_tool",
            "description": "Not in the reviewed profile catalogue.",
            "inputSchema": {"type": "object"},
        },
    )

    planned = activation.record_binding_plan(plan)
    snapshot = planned.to_dict()
    encoded = json.dumps(snapshot, sort_keys=True)

    assert snapshot["schema"] == AUTONOMOUS_ACTIVATION_SCHEMA
    assert {row["domain"] for row in snapshot["domain_statuses"]} == set(AUTONOMOUS_DOMAIN_NAMES)
    assert snapshot["status"] == "review_required"
    assert snapshot["approved_tools"] == []
    assert "tabular_ingest" in snapshot["pending_review_tools"]
    assert "unclassified_workspace_tool" in snapshot["pending_review_tools"]
    assert "opaque-id-must-not-persist" not in encoded
    assert "provider-secret-must-not-persist" not in encoded
    assert "credential_id" not in encoded
    assert "secret_value" not in encoded
    assert '"credentials"' not in encoded

    activated = activation.approve_bindings(plan, ["repository_catalog"], registered_tool_count=1)
    assert activated.status == "partially_activated"
    assert activated.approved_tools == ("repository_catalog",)
    assert activated.registered_tool_count == 1


def test_activation_requires_fresh_approval_after_catalogue_drift_and_rejects_revoked_mutation() -> None:
    activation = AutonomousCapabilityActivation(activation_id="activation-drift", clock=lambda: 20.0)
    activation.record_provider_statuses([_provider_status()])
    first_plan = _plan(
        {
            "name": "repository_catalog",
            "inputSchema": {"type": "object"},
        }
    )
    activation.record_binding_plan(first_plan)
    assert activation.approve_bindings(first_plan, ["repository_catalog"]).status == "ready"

    changed_plan = _plan(
        {
            "name": "repository_catalog",
            "description": "The live schema description changed.",
            "inputSchema": {"type": "object"},
        }
    )
    stale = activation.record_binding_plan(changed_plan)
    assert stale.status == "stale"
    assert stale.approved_tools == ()
    assert activation.approve_bindings(changed_plan, ["repository_catalog"]).status == "ready"

    activation.revoke(reason="operator_revoked_activation")
    with pytest.raises(AutonomousActivationError, match="revoked"):
        activation.record_provider_statuses([_provider_status()])


def test_activation_store_round_trips_and_rejects_tampered_state(tmp_path) -> None:
    activation = AutonomousCapabilityActivation(activation_id="activation-store", clock=lambda: 30.0)
    activation.record_provider_statuses([_provider_status()])
    plan = _plan(
        {
            "name": "repository_catalog",
            "inputSchema": {"type": "object"},
        }
    )
    activation.record_binding_plan(plan)
    activation.approve_bindings(plan, ["repository_catalog"])
    store = AutonomousCapabilityActivationStore(tmp_path / "activation.json")

    receipt = store.save(activation)
    restored = store.load()
    assert restored is not None
    assert receipt["state_digest"] == restored.state_digest
    assert restored.to_dict() == activation.to_dict()

    payload = json.loads((tmp_path / "activation.json").read_text(encoding="utf-8"))
    payload["state_digest"] = "0" * 64
    (tmp_path / "activation.json").write_bytes(canonical_bytes(payload))
    with pytest.raises(AutonomousActivationError, match="digest"):
        store.load()


def test_activation_store_compare_and_swap_fences_stale_writers_and_noncanonical_json(tmp_path) -> None:
    activation = AutonomousCapabilityActivation(activation_id="activation-cas", clock=lambda: 30.0)
    store = AutonomousCapabilityActivationStore(tmp_path / "activation-cas.json")

    assert store.save_if_unchanged(activation, None) is True
    initial = store.load()
    assert initial is not None
    assert store.save_if_unchanged(initial, None) is False

    activation.revoke(reason="operator_revoked_activation")
    assert store.save_if_unchanged(activation, initial.state_digest) is True
    assert store.save_if_unchanged(initial, initial.state_digest) is False
    with pytest.raises(AutonomousActivationError, match="expected_state_digest"):
        store.save_if_unchanged(initial, "not-a-digest")

    payload = json.loads((tmp_path / "activation-cas.json").read_text(encoding="utf-8"))
    (tmp_path / "activation-cas.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    with pytest.raises(AutonomousActivationError, match="canonical"):
        store.load()
