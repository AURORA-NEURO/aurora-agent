from __future__ import annotations

import asyncio
import hashlib
import json

from prism_sdk import (
    AsyncBrainControlClient,
    AsyncDurableBrainControlPlaneAdapter,
    AUTONOMOUS_DOMAINS,
    BrainControlClient,
    BrainControlRefusal,
    DurableBrainControlPlaneAdapter,
)
from prism_sdk.jobs import BrainJobStore


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _submit_arguments(job_id: str, domain: str) -> dict[str, object]:
    return {
        "job_id": job_id,
        "idempotency_key": f"transport-{job_id}",
        "spec_digest": _digest(f"spec-{job_id}"),
        "domain": domain,
        "capability": "bounded_metadata_execution",
        "risk_class": "reversible_review",
        "priority": 10,
        "max_attempts": 3,
        "checkpoint_digest": _digest(f"checkpoint-{job_id}"),
    }


def test_durable_transport_fails_closed_and_rejects_non_contract_fields(tmp_path) -> None:
    definitions = DurableBrainControlPlaneAdapter.tool_definitions()
    assert tuple(definition["name"] for definition in definitions) == DurableBrainControlPlaneAdapter.TOOL_NAMES
    assert all(definition["inputSchema"]["additionalProperties"] is False for definition in definitions)

    with BrainJobStore(tmp_path / "transport.sqlite3") as store:
        locked = DurableBrainControlPlaneAdapter(store)
        refused = locked.call_tool("brain_job_submit", _submit_arguments("job-locked", "coding"))
        assert refused["ok"] is False
        assert refused["error"] == "authorization_required"

        def allow(_operation: str, metadata: dict[str, object]) -> bool:
            assert "api_key" not in json.dumps(metadata)
            return True

        adapter = DurableBrainControlPlaneAdapter(store, authorizer=allow)
        malformed = adapter.call_tool(
            "brain_job_submit",
            {**_submit_arguments("job-malformed", "coding"), "prompt": "never retained"},
        )
        assert malformed["ok"] is False
        assert malformed["error"] == "operation_refused"


def test_durable_transport_covers_every_domain_and_survives_restart(tmp_path) -> None:
    path = tmp_path / "durable-transport.sqlite3"
    authorization_calls: list[tuple[str, dict[str, object]]] = []

    def allow(operation: str, metadata: dict[str, object]) -> bool:
        authorization_calls.append((operation, dict(metadata)))
        assert "provider-secret" not in json.dumps(metadata)
        return True

    with BrainJobStore(path) as store:
        adapter = DurableBrainControlPlaneAdapter(store, authorizer=allow, principal="test-operator")
        for index, domain in enumerate(AUTONOMOUS_DOMAINS):
            job_id = f"job-domain-{index}"
            submitted = adapter.call_tool("brain_job_submit", _submit_arguments(job_id, domain))
            assert submitted["ok"] is True
            assert submitted["job"]["domain"] == domain  # type: ignore[index]
            assert "transport-secret" not in json.dumps(submitted)

        client = BrainControlClient(adapter.call_tool)
        submitted = client.submit_job(_submit_arguments("job-lifecycle", "engineering"))
        claimed = client.claim_job({"job_id": "job-lifecycle", "worker_id": "worker-a", "lease_ms": 10_000})
        assert claimed["job"]["state"] == "leased"
        client.checkpoint_job(
            {
                "job_id": "job-lifecycle",
                "worker_id": "worker-a",
                "phase": "preflight",
                "checkpoint_digest": _digest("preflight"),
                "side_effect_boundary": "preflight",
            }
        )
        completed = client.complete_job(
            {"job_id": "job-lifecycle", "worker_id": "worker-a", "result_digest": _digest("result")}
        )
        assert completed["job"]["state"] == "succeeded"
        assert completed["job"]["result_digest"] == _digest("result")
        assert submitted["job"]["idempotency_key_digest"] == hashlib.sha256(
            json.dumps("transport-job-lifecycle", separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        ).hexdigest()

        approval = client.submit_job(_submit_arguments("job-approval", "enterprise"))
        requested = client.approval({"job_id": "job-approval", "action": "request", "reason": "requires review"})
        assert requested["job"]["state"] == "waiting_approval"
        assert requested["authorization"]["verified_by_server"] is False
        client.approval(
            {
                "job_id": "job-approval",
                "action": "approve",
                "reason": "operator approved",
                "authorization_digest": _digest("operator-approval"),
            }
        )
        assert client.job_status("job-approval")["job"]["state"] == "queued"
        assert approval["job"]["job_id"] == "job-approval"

        adapter.call_tool("brain_job_submit", _submit_arguments("job-reconcile", "operations"))
        adapter.call_tool("brain_job_claim", {"job_id": "job-reconcile", "worker_id": "worker-r"})
        adapter.call_tool(
            "brain_job_checkpoint",
            {
                "job_id": "job-reconcile",
                "worker_id": "worker-r",
                "phase": "dispatch",
                "checkpoint_digest": _digest("dispatch"),
                "side_effect_boundary": "dispatched",
            },
        )
        failed = adapter.call_tool(
            "brain_job_fail",
            {
                "job_id": "job-reconcile",
                "worker_id": "worker-r",
                "reason": "provider-secret must never appear in output",
            },
        )
        assert failed["job"]["state"] == "reconciliation_required"
        reconciled = adapter.call_tool(
            "brain_job_reconcile",
            {
                "job_id": "job-reconcile",
                "outcome": "not_executed",
                "evidence_digest": _digest("no-effect-proof"),
                "effect_absent": True,
                "reason": "caller verified no external effect",
            },
        )
        assert reconciled["job"]["state"] == "queued"
        assert "provider-secret" not in json.dumps(failed)
        assert "provider-secret" not in json.dumps(reconciled)

        events = client.job_events({"job_id": "job-lifecycle", "limit": 64})
        assert events["events"]
        assert all("payload_digest" in event for event in events["events"])
        assert "provider-secret" not in json.dumps(events)
        assert store.verify_integrity()["ok"] is True

    with BrainJobStore(path) as reopened:
        adapter = DurableBrainControlPlaneAdapter(reopened, authorizer=allow, principal="test-operator")
        status = adapter.call_tool("brain_job_status", {"job_id": "job-lifecycle"})
        assert status["job"]["state"] == "succeeded"
        assert status["durability"]["restart"] == "durable_brain_job_store"
        page = adapter.call_tool("brain_job_events", {"after": 0, "limit": 256})
        assert page["events"]
        assert page["head_digest"] == reopened.head_digest()

    assert len(authorization_calls) >= len(AUTONOMOUS_DOMAINS)


def test_typed_client_preserves_durable_refusal_envelope(tmp_path) -> None:
    with BrainJobStore(tmp_path / "refusal.sqlite3") as store:
        client = BrainControlClient(DurableBrainControlPlaneAdapter(store).call_tool)
        try:
            client.submit_job(_submit_arguments("job-refused", "research"))
        except BrainControlRefusal as error:
            assert error.payload["error"] == "authorization_required"
        else:
            raise AssertionError("missing typed refusal")


def test_async_typed_client_uses_the_same_durable_store(tmp_path) -> None:
    async def run() -> None:
        with BrainJobStore(tmp_path / "async-transport.sqlite3") as store:
            adapter = DurableBrainControlPlaneAdapter(store, authorizer=lambda _operation, _metadata: True)
            async_adapter = AsyncDurableBrainControlPlaneAdapter(adapter)
            client = AsyncBrainControlClient.from_durable(async_adapter)
            receipt = await client.submit_job(_submit_arguments("job-async", "neuroscience"))
            assert receipt["job"]["state"] == "queued"
            status = await client.job_status("job-async")
            assert status["durability"]["scope"] == "python_sqlite"

    asyncio.run(run())
