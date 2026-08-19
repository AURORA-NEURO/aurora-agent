from __future__ import annotations

import asyncio
import hashlib
import json

import pytest

from prism_sdk import (
    AsyncBrainControlClient,
    BrainApprovalCommand,
    BrainControlClient,
    BrainControlError,
    BrainEventPageRequest,
    BrainHealthObservation,
    BrainJobSubmission,
    BrainReplayRequest,
)


def test_typed_job_requests_never_emit_task_or_secret_fields() -> None:
    request = BrainJobSubmission(
        idempotency_key="request-001",
        spec_digest="a" * 64,
        domain="engineering",
        capability="code_change",
        risk_class="reversible",
    )
    arguments = request.to_arguments()
    assert arguments["spec_digest"] == "a" * 64
    assert "task" not in arguments
    assert "prompt" not in arguments
    assert "api_key" not in arguments

    with pytest.raises((TypeError, BrainControlError)):
        BrainJobSubmission(
            idempotency_key="request-001",
            spec_digest="a" * 64,
            domain="engineering",
            capability="code_change",
            risk_class="reversible",
            **{"api_key": "must-refuse"},
        )


def test_replay_request_computes_cross_language_digest_and_rejects_tampering() -> None:
    request = BrainReplayRequest(
        case_id="case-001",
        domain="engineering",
        capability="code_change",
        risk_class="reversible",
        signals={"schema_valid": True, "tests_passed": True, "evidence_complete": True},
    )
    expected_evidence = {
        "schema": "bioprism-brain-domain-evaluator/0.1",
        "domain": "engineering",
        "capability": "code_change",
        "risk_class": "reversible",
        "signals": {"schema_valid": 1.0, "tests_passed": 1.0, "evidence_complete": 1.0},
        "references": [],
        "limitations": [],
        "retention": "value_only_digests_and_signal_scores",
    }
    encoded = json.dumps(
        expected_evidence,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    assert request.evidence_digest == hashlib.sha256(encoded).hexdigest()
    assert request.to_arguments()["signals"]["schema_valid"] == 1.0

    with pytest.raises(BrainControlError):
        BrainReplayRequest(
            case_id="case-001",
            domain="engineering",
            capability="code_change",
            risk_class="reversible",
            signals={"schema_valid": True, "tests_passed": True, "evidence_complete": True},
            evidence_digest="b" * 64,
        )


def test_sync_facade_adapts_http_style_transport_and_typed_commands() -> None:
    calls: list[tuple[str, dict[str, object]]] = []

    def transport(name: str, arguments: dict[str, object]) -> dict[str, object]:
        calls.append((name, arguments))
        return {"ok": True, "tool": name}

    client = BrainControlClient(transport)
    client.submit_job(
        BrainJobSubmission(
            idempotency_key="request-001",
            spec_digest="a" * 64,
            domain="data",
            capability="transform",
            risk_class="reversible",
        )
    )
    client.approval(
        BrainApprovalCommand(
            job_id="job-001",
            action="approve",
            authorization_digest="b" * 64,
        )
    )
    client.record_health(
        BrainHealthObservation(
            provider="openai",
            model="gpt-test",
            status="success",
            quality=0.9,
            credential_ready=True,
        )
    )
    client.job_events(BrainEventPageRequest(limit=2))
    assert [name for name, _ in calls] == [
        "brain_job_submit",
        "brain_job_approval",
        "brain_model_health",
        "brain_job_events",
    ]
    assert "authorization_digest" in calls[1][1]
    assert "secret" not in json.dumps(calls)


def test_async_facade_adapts_awaitable_transport() -> None:
    calls: list[str] = []

    async def transport(name: str, arguments: dict[str, object]) -> dict[str, object]:
        calls.append(name)
        return {"ok": True, "tool": name}

    async def run() -> None:
        client = AsyncBrainControlClient(transport)
        await client.health_snapshot()
        await client.replay(
            BrainReplayRequest(
                case_id="case-001",
                domain="research",
                capability="synthesis",
                risk_class="low",
                signals={
                    "evidence_traceable": True,
                    "uncertainty_reported": True,
                    "claim_scope_respected": True,
                },
            )
        )

    asyncio.run(run())
    assert calls == ["brain_model_health", "brain_replay_evaluate"]
