from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousClaimIntegrityClaim,
    AutonomousClaimIntegrityEvidence,
    AutonomousClaimIntegrityPolicy,
    LLMRuntime,
    assess_autonomous_claim_integrity,
    content_digest,
    reassess_autonomous_claim_integrity,
    validate_autonomous_claim_integrity,
    validate_autonomous_claim_integrity_snapshot,
)
from prism_sdk.errors import ArgumentError


REFERENCE = "2026-08-26T12:00:00Z"


def digest(value: str) -> str:
    return content_digest({"value": value})


def claim(claim_id: str, domain: str, **kwargs: object) -> AutonomousClaimIntegrityClaim:
    return AutonomousClaimIntegrityClaim(
        claim_id=claim_id,
        domain=domain,
        claim_digest=digest(f"claim:{claim_id}"),
        **kwargs,
    )


def evidence(
    evidence_id: str,
    claim_id: str,
    domain: str,
    *,
    source: str | None = None,
    observed_at: str = "2026-08-25T12:00:00Z",
    reliability: float = 0.9,
    support: float = 0.9,
    stance: str = "support",
    modality: str = "primary",
    reproducibility: str = "reproduced",
    status: str = "accepted",
    **kwargs: object,
) -> AutonomousClaimIntegrityEvidence:
    return AutonomousClaimIntegrityEvidence(
        evidence_id=evidence_id,
        domain=domain,
        claim_ids=(claim_id,),
        source_id=source or f"source-{evidence_id}",
        source_digest=digest(f"source:{source or evidence_id}"),
        evidence_digest=digest(f"evidence:{evidence_id}"),
        observed_at=observed_at,
        reliability=reliability,
        support=support,
        stance=stance,
        modality=modality,
        reproducibility=reproducibility,
        status=status,
        **kwargs,
    )


def test_integrity_fuses_all_domains_without_retaining_claim_text() -> None:
    claims = tuple(claim(f"claim-{domain}", domain) for domain in AUTONOMOUS_DOMAINS)
    evidence_rows = tuple(
        evidence(f"evidence-{domain}", f"claim-{domain}", domain)
        for domain in AUTONOMOUS_DOMAINS
    )

    result = assess_autonomous_claim_integrity(
        context_digest=digest("all-domain-task"),
        claims=claims,
        evidence=evidence_rows,
        reference_time=REFERENCE,
        policy=AutonomousClaimIntegrityPolicy(min_support=0.5),
    )

    assert result.status == "ready"
    assert result.ready
    assert result.summary["supported_claim_count"] == len(AUTONOMOUS_DOMAINS)
    assert result.actions == ()
    projection = result.to_dict()
    assert projection["execution"].startswith("provider_free")
    assert projection["secret_material"] == "never_returned"
    assert "all-domain-task" not in str(projection)


def test_integrity_makes_temporal_conflict_independence_modal_and_reproduction_actions_explicit() -> None:
    claims = (
        claim("conflict", "science"),
        claim("stale", "coding"),
        claim("independent", "data", required_independent_sources=2),
        claim("modal", "multimodal", required_modalities=("imaging", "omics")),
        claim("repro", "evaluation", required_reproducibility=True),
    )
    evidence_rows = (
        evidence("conflict-support", "conflict", "science", source="source-a"),
        evidence("conflict-contradiction", "conflict", "science", source="source-b", stance="contradict"),
        evidence("stale-evidence", "stale", "coding", observed_at="2026-01-01T12:00:00Z"),
        evidence("one-source", "independent", "data", source="single-source"),
        evidence("imaging-only", "modal", "multimodal", modality="imaging"),
        evidence("observed-only", "repro", "evaluation", reproducibility="observed"),
    )

    result = assess_autonomous_claim_integrity(
        context_digest=digest("blocked-task"),
        claims=claims,
        evidence=evidence_rows,
        reference_time=REFERENCE,
        policy={"require_cross_modal_agreement": False, "max_actions": 10},
    )
    by_id = {item.claim_id: item for item in result.claims}
    assert by_id["conflict"].status == "conflicted"
    assert by_id["conflict"].next_action_type == "resolve_contradiction"
    assert by_id["stale"].status == "stale"
    assert by_id["stale"].next_action_type == "acquire_fresh_evidence"
    assert by_id["independent"].status == "insufficient_independence"
    assert by_id["modal"].status == "insufficient_modalities"
    assert by_id["repro"].status == "unreproducible"
    assert result.status == "blocked"
    assert {action.action_type for action in result.actions} == {
        "resolve_contradiction",
        "acquire_fresh_evidence",
        "acquire_independent_source",
        "acquire_cross_modal_evidence",
        "reproduce_evidence",
    }


def test_integrity_reassessment_is_generation_and_digest_fenced() -> None:
    initial_claim = claim("recover", "science")
    first = assess_autonomous_claim_integrity(
        context_digest=digest("recover-task"),
        claims=(initial_claim,),
        evidence=(),
        reference_time=REFERENCE,
    )
    assert first.status == "blocked"

    second = reassess_autonomous_claim_integrity(
        first,
        claims=(initial_claim,),
        evidence=(evidence("recovered", "recover", "science"),),
        reference_time=REFERENCE,
    )
    assert second.generation == 2
    assert second.prior_assessment_digest == first.assessment_digest
    assert second.claims[0].status == "supported"
    assert validate_autonomous_claim_integrity(second) is second
    assert validate_autonomous_claim_integrity_snapshot(second.to_dict())["assessment_digest"] == second.assessment_digest

    wire = second.to_dict()
    wire["summary"] = {"tampered": True}
    with pytest.raises(ArgumentError):
        validate_autonomous_claim_integrity_snapshot(wire)


def test_integrity_temporal_firewall_and_secret_metadata_fail_closed() -> None:
    with pytest.raises(ArgumentError):
        claim("secret", "coding", metadata={"api_key": "never accepted"})
    future = evidence("future", "future-claim", "coding", observed_at="2027-01-01T00:00:00Z")
    expired = evidence("expired", "expired-claim", "science", valid_until="2026-08-01T00:00:00Z")
    result = assess_autonomous_claim_integrity(
        context_digest=digest("temporal-task"),
        claims=(claim("future-claim", "coding"), claim("expired-claim", "science")),
        evidence=(future, expired),
        reference_time=REFERENCE,
    )
    by_id = {item.evidence_id: item for item in result.evidence}
    assert by_id["future"].temporal_state == "future"
    assert by_id["future"].usable is False
    assert by_id["expired"].temporal_state == "expired"
    assert all(item.status in {"blocked", "missing"} for item in result.claims)


def test_agent_facade_binds_task_digest_without_provider_or_source_dispatch() -> None:
    agent = AutonomousAgent(object(), LLMRuntime())
    result = agent.assess_claim_integrity(
        task="decide whether a bounded science claim may be used",
        claims=(claim("science-claim", "science"),),
        evidence=(evidence("science-evidence", "science-claim", "science"),),
        reference_time=REFERENCE,
    )
    assert result.status == "ready"
    assert result.to_dict()["context_digest"] == content_digest({"task": "decide whether a bounded science claim may be used"})
    assert "decide whether" not in str(result.to_dict())
