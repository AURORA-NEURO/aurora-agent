from __future__ import annotations

from types import SimpleNamespace

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousClaimIntegrityClaim,
    AutonomousClaimIntegrityEvidence,
    AutonomousClaimIntegrityPolicy,
    AutonomousOutcomeIntegrityRun,
    LLMRuntime,
    assess_autonomous_outcome_integrity,
    bind_autonomous_outcome_integrity_claims,
    content_digest,
    validate_autonomous_outcome_integrity,
    validate_autonomous_outcome_integrity_snapshot,
)
from prism_sdk.errors import ArgumentError


REFERENCE = "2026-08-26T12:00:00Z"


def digest(value: str) -> str:
    return content_digest({"value": value})


def run(**overrides: object) -> AutonomousOutcomeIntegrityRun:
    values: dict[str, object] = {
        "task_digest": digest("outcome-task"),
        "route_digest": digest("route"),
        "status": "completed",
        "mode": "single_domain",
        "domains": ("science",),
        "output_digest": digest("answer"),
        "response_digest": digest("response"),
        "outcome_digest": digest("outcome"),
    }
    values.update(overrides)
    return AutonomousOutcomeIntegrityRun(**values)  # type: ignore[arg-type]


def claim(claim_id: str = "claim-1") -> AutonomousClaimIntegrityClaim:
    return AutonomousClaimIntegrityClaim(
        claim_id=claim_id,
        domain="science",
        claim_digest=digest(f"claim:{claim_id}"),
    )


def evidence(claim_id: str = "claim-1") -> AutonomousClaimIntegrityEvidence:
    return AutonomousClaimIntegrityEvidence(
        evidence_id="evidence-1",
        domain="science",
        claim_ids=(claim_id,),
        source_id="source-1",
        source_digest=digest("source-1"),
        evidence_digest=digest("evidence-1"),
        observed_at="2026-08-25T12:00:00Z",
        reliability=0.9,
        support=0.9,
        status="accepted",
        stance="support",
        modality="primary",
        reproducibility="reproduced",
    )


def binding(**overrides: object) -> dict[str, object]:
    value: dict[str, object] = {
        "claim_id": "claim-1",
        "domain": "science",
        "role": "run_output",
        "output_digest": digest("answer"),
        "response_digest": digest("response"),
    }
    value.update(overrides)
    return value


def test_outcome_integrity_emits_a_ready_metadata_only_reliance_contract() -> None:
    result = assess_autonomous_outcome_integrity(
        run=run(),
        claims=(claim(),),
        evidence=(evidence(),),
        claim_bindings=(binding(),),
        reference_time=REFERENCE,
        policy=AutonomousClaimIntegrityPolicy(min_support=0.5),
    )
    assert result.status == "ready"
    assert result.gate_reasons == ()
    assert result.next_actions == ()
    assert result.claim_count == 1
    assert result.evidence_count == 1
    assert result.run.output_digest == digest("answer")
    assert result.to_dict()["secret_material"] == "never_returned"
    assert "outcome-task" not in str(result.to_dict())
    assert validate_autonomous_outcome_integrity(result) is result
    assert validate_autonomous_outcome_integrity_snapshot(result.to_dict())["assessment_digest"] == result.assessment_digest


def test_outcome_integrity_covers_every_built_in_domain_with_deterministic_order() -> None:
    claims = tuple(
        AutonomousClaimIntegrityClaim(
            claim_id=f"claim-{domain}",
            domain=domain,
            claim_digest=digest(f"claim:{domain}"),
        )
        for domain in AUTONOMOUS_DOMAINS
    )
    evidence_rows = tuple(
        AutonomousClaimIntegrityEvidence(
            evidence_id=f"evidence-{domain}",
            domain=domain,
            claim_ids=(f"claim-{domain}",),
            source_id=f"source-{domain}",
            source_digest=digest(f"source:{domain}"),
            evidence_digest=digest(f"evidence:{domain}"),
            observed_at="2026-08-25T12:00:00Z",
            reliability=0.9,
            support=0.9,
            status="accepted",
            stance="support",
            modality="primary",
            reproducibility="reproduced",
        )
        for domain in AUTONOMOUS_DOMAINS
    )
    projected_run = run(mode="cross_domain", domains=tuple(AUTONOMOUS_DOMAINS))
    bindings = tuple(
        binding(
            claim_id=f"claim-{domain}",
            domain=domain,
            role="synthesis_response" if domain == "cross_domain" else "specialist_response",
        )
        for domain in AUTONOMOUS_DOMAINS
    )
    result = assess_autonomous_outcome_integrity(
        run=projected_run,
        claims=claims,
        evidence=evidence_rows,
        claim_bindings=bindings,
        reference_time=REFERENCE,
        policy={"min_support": 0.5},
    )
    assert result.status == "ready"
    assert result.claim_count == len(AUTONOMOUS_DOMAINS)
    assert result.run.domains == tuple(AUTONOMOUS_DOMAINS)


def test_outcome_integrity_blocks_incomplete_runs_and_missing_bindings() -> None:
    result = assess_autonomous_outcome_integrity(
        run=run(status="approval_required"),
        claims=(claim(),),
        evidence=(evidence(),),
        claim_bindings=(),
        reference_time=REFERENCE,
    )
    assert result.status == "blocked"
    assert "run_not_completed" in result.gate_reasons
    assert "claim_bindings_incomplete" in result.gate_reasons
    assert "inspect_incomplete_run" in result.next_actions
    assert "rebind_claims_to_exact_run_output" in result.next_actions


def test_outcome_integrity_requires_cross_domain_synthesis_review_when_requested() -> None:
    result = assess_autonomous_outcome_integrity(
        run=run(mode="cross_domain", domains=("science", "data", "cross_domain")),
        claims=(claim(),),
        evidence=(evidence(),),
        claim_bindings=(binding(),),
        reference_time=REFERENCE,
        require_response_assessment=True,
        require_synthesis=True,
    )
    assert result.status == "blocked"
    assert "response_assessment_missing" in result.gate_reasons
    assert "synthesis_not_completed" in result.gate_reasons


def test_outcome_integrity_rejects_output_drift_and_tampering() -> None:
    exact_run = run()
    with pytest.raises(ArgumentError):
        bind_autonomous_outcome_integrity_claims(exact_run, (binding(output_digest=digest("other-answer")),))
    result = assess_autonomous_outcome_integrity(
        run=exact_run,
        claims=(claim(),),
        evidence=(evidence(),),
        claim_bindings=(binding(),),
        reference_time=REFERENCE,
    )
    tampered = result.to_dict()
    tampered["claim_count"] = 99
    with pytest.raises(ArgumentError):
        validate_autonomous_outcome_integrity_snapshot(tampered)


def test_autonomous_facade_projects_a_transient_direct_result_without_provider_dispatch() -> None:
    raw = SimpleNamespace(
        status="completed",
        route=SimpleNamespace(task_digest=digest("facade-task"), route_digest=digest("facade-route")),
        blueprint=SimpleNamespace(domain_pack=SimpleNamespace(domain="science")),
        response=SimpleNamespace(text="answer", structured=None),
        outcome_digest=digest("outcome"),
    )
    agent = AutonomousAgent(None, LLMRuntime())
    projected = agent.project_outcome_integrity_run(raw)
    result = agent.assess_outcome_integrity(
        raw,
        claims=(claim(),),
        evidence=(evidence(),),
        claim_bindings=(binding(output_digest=projected.output_digest, response_digest=projected.response_digest),),
        reference_time=REFERENCE,
    )
    assert result.status == "ready"
    assert result.run.task_digest == digest("facade-task")
