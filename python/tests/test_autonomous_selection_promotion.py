from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    ArgumentError,
    evaluate_autonomous_selection_policy,
    evaluate_autonomous_selection_promotion,
    validate_autonomous_selection_promotion_report,
)


def _request(domain: str) -> dict:
    return {
        "task": f"private promotion task for {domain}; this text must never be retained",
        "domain": domain,
        "capability": "default",
        "risk_class": "low",
        "required_capabilities": [],
        "estimated_input_tokens": 100,
        "requested_output_tokens": 100,
        "candidates": [
            {
                "provider": "lab",
                "model": f"{domain}-quality",
                "capabilities": ["structured_output"],
                "context_window_tokens": 8192,
                "max_output_tokens": 2048,
                "quality": 0.9,
                "latency_ms": 20,
                "cost_per_million_tokens": 4,
                "reliability": 0.92,
                "enabled": True,
            },
            {
                "provider": "lab",
                "model": f"{domain}-cheap",
                "capabilities": ["structured_output"],
                "context_window_tokens": 8192,
                "max_output_tokens": 2048,
                "quality": 0.7,
                "latency_ms": 10,
                "cost_per_million_tokens": 1,
                "reliability": 0.85,
                "enabled": True,
            },
        ],
        "provider_health": {
            "lab": {
                "provider": "lab",
                "circuit": "closed",
                "consecutive_failures": 0,
                "attempts": 0,
                "successes": 0,
                "failures": 0,
                "success_rate": 0,
                "credential_required": False,
                "credential_ready": True,
                "structured_output_mode": "json_object",
            }
        },
        "model_health": {},
    }


def _cases(*, selected_arm_wins: bool = True) -> list[dict]:
    return [
        {
            "case_id": f"{domain}-promotion-case",
            "domain": domain,
            "request": _request(domain),
            "rewards": (
                {f"lab/{domain}-quality": 0.95, f"lab/{domain}-cheap": 0.25}
                if selected_arm_wins
                else {f"lab/{domain}-quality": 0.25, f"lab/{domain}-cheap": 0.95}
            ),
        }
        for domain in AUTONOMOUS_DOMAINS
    ]


def test_selection_promotion_admits_complete_high_agreement_policy() -> None:
    replay = evaluate_autonomous_selection_policy(_cases(), require_all_domains=True)
    promotion = evaluate_autonomous_selection_promotion(replay)

    assert promotion["decision"] == "admit"
    assert promotion["reasons"] == []
    assert len(promotion["domains"]) == len(AUTONOMOUS_DOMAINS)
    assert all(row["decision"] == "admit" and row["oracle_agreement_rate"] == 1 and row["mean_regret"] == 0 for row in promotion["domains"])
    assert "private promotion task" not in str(promotion)
    assert validate_autonomous_selection_promotion_report(promotion) == promotion
    assert evaluate_autonomous_selection_promotion(replay)["promotion_digest"] == promotion["promotion_digest"]


def test_selection_promotion_holds_incomplete_and_low_quality_evidence() -> None:
    incomplete_replay = evaluate_autonomous_selection_policy([_cases()[0]], require_all_domains=True)
    incomplete = evaluate_autonomous_selection_promotion(incomplete_replay)
    assert incomplete["decision"] == "hold"
    assert "selection replay report is not complete" in incomplete["reasons"]
    assert sum(row["decision"] == "hold" for row in incomplete["domains"]) == len(AUTONOMOUS_DOMAINS) - 1

    low_quality_replay = evaluate_autonomous_selection_policy(_cases(selected_arm_wins=False), require_all_domains=True)
    low_quality = evaluate_autonomous_selection_promotion(low_quality_replay)
    assert low_quality["decision"] == "hold"
    assert all(any("oracle agreement" in reason for reason in row["reasons"]) and row["mean_regret"] is not None for row in low_quality["domains"])
    assert low_quality["domains"][0]["mean_regret"] == 0.7


def test_selection_promotion_validates_bounds_and_digest_tampering() -> None:
    replay = evaluate_autonomous_selection_policy(_cases(), require_all_domains=True)
    with pytest.raises(ArgumentError, match="max_mean_regret"):
        evaluate_autonomous_selection_promotion(replay, max_mean_regret=3)

    promotion = evaluate_autonomous_selection_promotion(replay)
    with pytest.raises(ArgumentError, match="decision|digest"):
        validate_autonomous_selection_promotion_report({**promotion, "decision": "hold"})
    with pytest.raises(ArgumentError, match="digest|decision|reasons"):
        validate_autonomous_selection_promotion_report({
            **promotion,
            "domains": [
                {**row, "reasons": ["tampered"]} if index == 0 else row
                for index, row in enumerate(promotion["domains"])
            ],
        })
