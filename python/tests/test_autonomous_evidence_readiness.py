from __future__ import annotations

import json

import pytest

from test_autonomy import _Workspace

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    BrainRunError,
    AutonomousLLMEvidenceAdapterRegistry,
    AutonomousLLMEvidenceReadinessAuditor,
    AutonomousLLMEvidenceReadinessPolicy,
    AutonomousLLMEvidenceReadinessReport,
    InMemoryAutonomousLLMEvidenceAdapterHealthStore,
    LLMRuntime,
    create_autonomous_llm_evidence_adapter,
)
from prism_sdk.errors import ArgumentError


def _adapter(runtime: LLMRuntime, domain: str, adapter_id: str):
    return create_autonomous_llm_evidence_adapter(
        adapter_id=adapter_id,
        version="v1",
        domain=domain,
        provider=f"readiness-{adapter_id}",
        runtime=runtime,
        capabilities=("llm_evidence",),
        model="offline-readiness-model",
        prompt_for_context=lambda _context: [{"role": "user", "content": "offline"}],
        require_json=True,
    )


def _registry(runtime: LLMRuntime, *, secondary: bool = True) -> AutonomousLLMEvidenceAdapterRegistry:
    adapters = []
    for domain in AUTONOMOUS_DOMAINS:
        adapters.append(_adapter(runtime, domain, f"a-primary-{domain}"))
        if secondary:
            adapters.append(_adapter(runtime, domain, f"b-secondary-{domain}"))
    return AutonomousLLMEvidenceAdapterRegistry(adapters)


def _record_successes(
    health: InMemoryAutonomousLLMEvidenceAdapterHealthStore,
    registry: AutonomousLLMEvidenceAdapterRegistry,
    *,
    adapter_prefix: str = "a-primary-",
    attempts: int = 1,
) -> None:
    for domain in AUTONOMOUS_DOMAINS:
        manifest = registry.manifest_for(domain, f"{adapter_prefix}{domain}")
        for _ in range(attempts):
            health.record_acquisition(
                adapter_id=manifest.adapter_id,
                manifest_digest=manifest.manifest_digest,
                domain=domain,
                outcome="success",
                status="observed",
                latency_ms=4,
            )


def test_readiness_reports_missing_all_domain_coverage_and_round_trips_strictly() -> None:
    report = AutonomousLLMEvidenceReadinessAuditor(
        AutonomousLLMEvidenceAdapterRegistry()
    ).audit()

    assert report.status == "blocked"
    assert report.missing_count == len(AUTONOMOUS_DOMAINS)
    assert report.blocked_count == 0
    assert all(row.status == "missing" for row in report.domains)
    wire = report.to_dict()
    assert wire["execution"] == "readiness_projection_only;no_source_dispatch"
    assert wire["secret_material"] == "never_returned"
    assert "offline-readiness-model" not in json.dumps(wire)
    restored = AutonomousLLMEvidenceReadinessReport.from_dict(wire)
    assert restored.report_digest == report.report_digest
    assert restored.to_dict() == wire


def test_readiness_requires_health_by_default_but_can_project_degraded_startup() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime, secondary=False)
    auditor = AutonomousLLMEvidenceReadinessAuditor(registry)

    strict = auditor.audit()
    assert strict.status == "blocked"
    assert strict.blocked_count == len(AUTONOMOUS_DOMAINS)
    assert all(row.reason == "selected_adapter_has_no_usable_health_observation" for row in strict.domains)

    degraded = auditor.audit(policy=AutonomousLLMEvidenceReadinessPolicy(require_health=False))
    assert degraded.status == "degraded"
    assert degraded.degraded_count == len(AUTONOMOUS_DOMAINS)

    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    _record_successes(health, registry)
    ready = AutonomousLLMEvidenceReadinessAuditor(registry, health).audit()
    assert ready.status == "ready"
    assert ready.ready_count == len(AUTONOMOUS_DOMAINS)
    assert all(row.health.attempts == 1 for row in ready.domains)


def test_adaptive_readiness_promotes_healthy_secondary_and_blocks_open_primary() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime)
    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    for domain in AUTONOMOUS_DOMAINS:
        primary = registry.manifest_for(domain, f"a-primary-{domain}")
        secondary = registry.manifest_for(domain, f"b-secondary-{domain}")
        for _ in range(3):
            health.record_acquisition(
                adapter_id=primary.adapter_id,
                manifest_digest=primary.manifest_digest,
                domain=domain,
                outcome="failure",
                status="failed",
                latency_ms=40,
                failure_class="provider_retryable",
            )
            health.record_acquisition(
                adapter_id=secondary.adapter_id,
                manifest_digest=secondary.manifest_digest,
                domain=domain,
                outcome="success",
                status="observed",
                latency_ms=5,
            )

    report = AutonomousLLMEvidenceReadinessAuditor(registry, health).audit(
        adaptive_selection=True,
        policy=AutonomousLLMEvidenceReadinessPolicy(min_attempts=3),
    )
    assert report.status == "ready"
    assert all(row.selected_adapter_id and row.selected_adapter_id.startswith("b-secondary-") for row in report.domains)
    assert all(row.health.circuit == "closed" for row in report.domains)
    assert report.health_snapshot_digest is not None
    assert all(row.selection_strategy == "weighted_evidence" for row in report.domains)


def test_agent_readiness_integrates_evidence_audit_without_dispatch_and_rejects_tampering() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime, secondary=False)
    health = InMemoryAutonomousLLMEvidenceAdapterHealthStore()
    _record_successes(health, registry)
    report = AutonomousLLMEvidenceReadinessAuditor(registry, health).audit()
    tampered = json.loads(json.dumps(report.to_dict()))
    tampered["ready_count"] += 1
    with pytest.raises(ArgumentError, match="aggregates are inconsistent"):
        AutonomousLLMEvidenceReadinessReport.from_dict(tampered)

    provider_calls: list[object] = []
    runtime.register_in_memory_provider("unused", lambda request: provider_calls.append(request) or {"text": "unused"})
    agent = AutonomousAgent(_Workspace(), runtime)
    readiness = agent.readiness(
        evidence_readiness={
            "registry": registry,
            "health_store": health,
            "options": {"policy": AutonomousLLMEvidenceReadinessPolicy(min_attempts=1)},
        }
    )
    assert readiness["evidence"]["status"] == "ready"
    assert readiness["evidence"]["ready_count"] == len(AUTONOMOUS_DOMAINS)
    assert all(row["evidence_readiness"]["status"] == "ready" for row in readiness["domains"])
    assert provider_calls == []


def test_readiness_rejects_unknown_options_and_adaptive_selection_without_health() -> None:
    runtime = LLMRuntime()
    registry = _registry(runtime, secondary=False)
    auditor = AutonomousLLMEvidenceReadinessAuditor(registry)
    with pytest.raises(ArgumentError, match="requires a health store"):
        auditor.audit(adaptive_selection=True)
    agent = AutonomousAgent(_Workspace(), runtime)
    with pytest.raises(BrainRunError, match="audit was rejected"):
        agent.readiness(
            evidence_readiness={
                "registry": registry,
                "options": {"unsupported_option": True},
            }
        )
