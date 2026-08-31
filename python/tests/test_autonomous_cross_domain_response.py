from __future__ import annotations

import json
from itertools import combinations

import pytest

from prism_sdk.autonomous_cross_domain_response import (
    AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA,
    AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA,
    assess_autonomous_cross_domain_response_set,
    replay_autonomous_cross_domain_response_assessment,
    validate_autonomous_cross_domain_response_assessment,
)
from prism_sdk.autonomous_domain_response import (
    AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
    AutonomousDomainResponseContract,
    build_autonomous_domain_response_contract,
    evaluate_autonomous_domain_response,
)
from prism_sdk.autonomy import builtin_autonomous_domain_profiles, builtin_autonomous_workflow_strategies
from prism_sdk.errors import ArgumentError


def _contract_for(profile: object) -> AutonomousDomainResponseContract:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == profile.domain)
    return build_autonomous_domain_response_contract(profile, workflow=workflow)


def _response(contract: AutonomousDomainResponseContract, *, complete: bool = True) -> dict[str, object]:
    return {
        "schema": AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
        "domain": contract.domain,
        "workflow_id": contract.workflow_id,
        "status": "complete" if complete else "partial",
        "answer": f"Bounded answer for {contract.domain}." if complete else "Partial answer.",
        "observations": ["bounded observation"] if complete else [],
        "inferences": ["bounded inference"] if complete else [],
        "uncertainty": ["bounded uncertainty"] if complete else [],
        "evidence_gaps": ["bounded evidence gap"] if complete else [],
        "next_actions": ["review the next action"] if complete else [],
        "stages": [
            {
                "stage_id": stage_id,
                "status": "complete" if complete else "not_attempted",
                "evidence": [f"evidence:{stage_id}"] if complete else [],
                "findings": [f"finding:{stage_id}"] if complete else [],
                "uncertainty": [f"uncertainty:{stage_id}"] if complete else [],
                "open_questions": [],
            }
            for stage_id in contract.stage_ids
        ],
        "domain_details": {
            field: [f"detail:{field}"] if complete else []
            for field in contract.domain_fields
        },
        "retention": "transient_provider_response_only;validated_against_reviewed_domain_contract",
        "secret_material": "never_returned",
    }


def _entries(*, complete: bool = True, include_synthesis: bool = False) -> tuple[list[dict[str, object]], dict[str, AutonomousDomainResponseContract]]:
    profiles = {profile.domain: profile for profile in builtin_autonomous_domain_profiles()}
    specialist_domains = [domain for domain in profiles if domain != "cross_domain"]
    contracts = {domain: _contract_for(profile) for domain, profile in profiles.items()}
    entries = [
        {"domain": domain, "contract": contracts[domain], "response": _response(contracts[domain], complete=complete), "role": "specialist"}
        for domain in specialist_domains
    ]
    if include_synthesis:
        entries.append({"domain": "cross_domain", "contract": contracts["cross_domain"], "response": _response(contracts["cross_domain"]), "role": "synthesis"})
    return entries, contracts


def _alignments(entries: list[dict[str, object]], contracts: dict[str, AutonomousDomainResponseContract], *, stance: str = "support") -> list[dict[str, object]]:
    specialists = [entry["domain"] for entry in entries if entry["domain"] != "cross_domain"]
    digests = {
        domain: evaluate_autonomous_domain_response(
            next(entry for entry in entries if entry["domain"] == domain)["response"],
            contracts[domain],
        ).response_digest
        for domain in specialists
    }
    result: list[dict[str, object]] = []
    for index, (left, right) in enumerate(combinations(specialists, 2)):
        result.append({
            "alignment_id": f"alignment-{index:03d}",
            "left_domain": left,
            "right_domain": right,
            "stance": stance,
            "confidence": 0.95,
            "topic_digest": f"{index + 1:064x}"[-64:],
            "rationale_digest": f"{index + 10_000:064x}"[-64:],
            "left_response_digest": digests[left],
            "right_response_digest": digests[right],
        })
    return result


def test_all_specialist_domains_have_a_digest_bound_synthesis_gate() -> None:
    entries, contracts = _entries(include_synthesis=False)
    domains = [entry["domain"] for entry in entries]
    alignments = _alignments(entries, contracts)
    assessment = assess_autonomous_cross_domain_response_set(
        entries,
        requested_domains=domains,
        context_digest="a" * 64,
        alignments=alignments,
    )
    assert assessment.status == "ready_to_synthesize"
    assert assessment.ready_to_synthesize is True
    assert assessment.requested_domains == tuple(domains)
    assert len(assessment.rows) == len(domains)
    assert assessment.alignment_pairs_expected == len(alignments)
    assert assessment.alignment_pairs_observed == len(alignments)
    projection = assessment.to_dict()
    assert projection["schema"] == AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA
    assert projection["alignments"][0]["schema"] == AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA
    encoded = json.dumps(projection, sort_keys=True)
    assert "Bounded answer" not in encoded
    assert validate_autonomous_cross_domain_response_assessment(projection).assessment_digest == assessment.assessment_digest


def test_synthesis_row_completes_the_gate_and_is_replayable() -> None:
    entries, contracts = _entries(include_synthesis=True)
    specialist_entries = [entry for entry in entries if entry["domain"] != "cross_domain"]
    alignments = _alignments(specialist_entries, contracts)
    assessment = assess_autonomous_cross_domain_response_set(
        entries,
        requested_domains=[entry["domain"] for entry in specialist_entries],
        alignments=alignments,
        require_synthesis=True,
    )
    assert assessment.status == "completed"
    assert assessment.ready_to_synthesize is False
    assert assessment.synthesis_domain_present is True
    assert assessment.next_actions == ()
    replayed = replay_autonomous_cross_domain_response_assessment(
        entries,
        assessment,
        requested_domains=[entry["domain"] for entry in specialist_entries],
        alignments=alignments,
        require_synthesis=True,
    )
    assert replayed.assessment_digest == assessment.assessment_digest


def test_alignment_conflict_and_missing_coverage_stop_synthesis() -> None:
    entries, contracts = _entries(include_synthesis=False)
    selected = entries[:2]
    left, right = selected
    left_domain = left["domain"]
    right_domain = right["domain"]
    left_digest = evaluate_autonomous_domain_response(left["response"], contracts[left_domain]).response_digest
    right_digest = evaluate_autonomous_domain_response(right["response"], contracts[right_domain]).response_digest
    contradiction = {
        "alignment_id": "contradiction-1",
        "left_domain": left_domain,
        "right_domain": right_domain,
        "stance": "contradict",
        "confidence": 0.99,
        "topic_digest": "b" * 64,
        "rationale_digest": None,
        "left_response_digest": left_digest,
        "right_response_digest": right_digest,
    }
    assessment = assess_autonomous_cross_domain_response_set(
        selected,
        requested_domains=[left_domain, right_domain, "evaluation"],
        alignments=[contradiction],
    )
    assert assessment.status == "partial"
    assert assessment.ready_to_synthesize is False
    assert "evaluation" in assessment.missing_domains
    assert assessment.contradictory_alignment_ids == ("contradiction-1",)
    assert "resolve_cross_domain_contradiction" in assessment.next_actions

    low_confidence = dict(contradiction)
    low_confidence["alignment_id"] = "low-confidence-1"
    low_confidence["stance"] = "support"
    low_confidence["confidence"] = 0.5
    review = assess_autonomous_cross_domain_response_set(
        selected,
        requested_domains=[left_domain, right_domain],
        alignments=[low_confidence],
    )
    assert review.status == "needs_alignment_review"
    assert review.low_confidence_alignment_ids == ("low-confidence-1",)
    assert "review_low_confidence_cross_domain_alignment" in review.next_actions


def test_weak_responses_and_tampering_fail_closed_without_retaining_values() -> None:
    entries, contracts = _entries(complete=False, include_synthesis=False)
    selected = entries[:2]
    assessment = assess_autonomous_cross_domain_response_set(
        selected,
        requested_domains=[entry["domain"] for entry in selected],
        alignments=[],
    )
    assert assessment.status == "partial"
    assert "repair_domain_response_integrity" in assessment.next_actions
    projection = assessment.to_dict()
    projection["status"] = "ready_to_synthesize"
    with pytest.raises(ArgumentError, match="digest"):
        validate_autonomous_cross_domain_response_assessment(projection)

    secret = _response(contracts[selected[0]["domain"]])
    secret["domain_details"][contracts[selected[0]["domain"]].domain_fields[0]] = ["gsk_should_never_be_accepted"]
    with pytest.raises(ArgumentError, match="credential"):
        assess_autonomous_cross_domain_response_set([
            {"domain": selected[0]["domain"], "contract": contracts[selected[0]["domain"]], "response": secret, "role": "specialist"},
            selected[1],
        ], requested_domains=[selected[0]["domain"], selected[1]["domain"]])
