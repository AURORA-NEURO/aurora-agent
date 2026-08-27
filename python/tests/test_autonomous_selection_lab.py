from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    ArgumentError,
    DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS,
    evaluate_autonomous_selection_policy,
    normalize_autonomous_selection_weights,
    rank_autonomous_models,
    validate_autonomous_selection_lab_report,
)


def _request(domain: str, *, all_disabled: bool = False) -> dict:
    return {
        "task": f"private evaluator task for {domain}; this text must never be retained",
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
                "enabled": not all_disabled,
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
                "enabled": not all_disabled,
            },
        ],
        "provider_health": {
            "lab": {
                "provider": "lab",
                "circuit": "closed",
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


def _cases() -> list[dict]:
    return [
        {
            "case_id": f"{domain}-selection-case",
            "domain": domain,
            "request": _request(domain),
            "rewards": {
                f"lab/{domain}-quality": 0.25,
                f"lab/{domain}-cheap": 0.95,
            },
        }
        for domain in AUTONOMOUS_DOMAINS
    ]


def test_selection_lab_evaluates_every_domain_without_retaining_task_text() -> None:
    cases = _cases()
    report = evaluate_autonomous_selection_policy(cases, require_all_domains=True)

    assert report["status"] == "completed"
    assert report["case_count"] == len(AUTONOMOUS_DOMAINS)
    assert report["evaluated_case_count"] == len(AUTONOMOUS_DOMAINS)
    assert report["missing_domains"] == []
    assert report["oracle_agreement_count"] == 0
    assert report["total_regret"] == 8.4
    assert "private evaluator task" not in str(report)
    assert validate_autonomous_selection_lab_report(report) == report
    assert evaluate_autonomous_selection_policy(cases, require_all_domains=True)["report_digest"] == report["report_digest"]


def test_selection_lab_reports_coverage_abstention_missing_rewards_and_disabled_models() -> None:
    one_case = [_cases()[0]]
    incomplete = evaluate_autonomous_selection_policy(one_case, require_all_domains=True)
    assert incomplete["status"] == "insufficient_coverage"
    assert len(incomplete["missing_domains"]) == len(AUTONOMOUS_DOMAINS) - 1

    abstained = evaluate_autonomous_selection_policy(
        one_case,
        selector=lambda _request: {
            "selected_model": None,
            "strategy": "caller_selector",
            "ranking": [],
            "abstention_reason": "caller gate",
        },
    )
    assert abstained["abstained_case_count"] == 1
    assert abstained["cases"][0]["status"] == "abstained"

    missing = evaluate_autonomous_selection_policy([{**one_case[0], "rewards": {}}])
    assert missing["no_counterfactual_reward_count"] == 1
    assert missing["cases"][0]["status"] == "no_counterfactual_reward"

    disabled = evaluate_autonomous_selection_policy(
        [{**one_case[0], "request": _request("coding", all_disabled=True)}]
    )
    assert disabled["no_eligible_model_count"] == 1
    assert disabled["cases"][0]["status"] == "no_eligible_model"


def test_selection_lab_rejects_unknown_or_ineligible_selector_choices() -> None:
    case = [_cases()[0]]
    with pytest.raises(ArgumentError, match="unknown model arm"):
        evaluate_autonomous_selection_policy(
            case,
            selector=lambda _request: {
                "selected_model": {"provider": "lab", "model": "unknown"},
                "strategy": "caller_selector",
            },
        )

    with pytest.raises(ArgumentError, match="ineligible"):
        evaluate_autonomous_selection_policy(
            [{**case[0], "request": _request("coding", all_disabled=True)}],
            selector=lambda _request: {
                "selected_model": {"provider": "lab", "model": "coding-quality"},
                "strategy": "caller_selector",
            },
        )


def test_default_ranker_applies_health_and_structured_output_gates() -> None:
    request = _request("coding")
    request["require_json"] = True
    request["provider_health"]["lab"]["structured_output_mode"] = "disabled"
    ranking = rank_autonomous_models(request)
    assert ranking[0]["eligible"] is False
    assert "provider structured output is disabled" in ranking[0]["reasons"]


def test_weighted_ranker_matches_policy_contract_and_respects_learning_disable() -> None:
    request = _request("evaluation")
    request["weights"] = {"quality": 1, "reliability": 0, "cost": 0, "latency": 0, "exploration": 0}
    assert normalize_autonomous_selection_weights() == DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS
    assert normalize_autonomous_selection_weights({"cost": 2})["cost"] == 2
    with pytest.raises(ArgumentError, match="at least one positive"):
        normalize_autonomous_selection_weights({name: 0 for name in DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS})
    assert rank_autonomous_models(request)[0]["model"] == "evaluation-quality"

    request["weights"] = {"quality": 0.1, "reliability": 0, "cost": 10, "latency": 0, "exploration": 0}
    assert rank_autonomous_models(request)[0]["model"] == "evaluation-cheap"

    request["observations"] = [
        {
            "arm_id": "lab/evaluation-cheap",
            "pulls": 12,
            "reward_sum": 10,
            "failures": 0,
            "disabled": True,
        }
    ]
    ranking = rank_autonomous_models(request)
    cheap = next(row for row in ranking if row["model"] == "evaluation-cheap")
    assert cheap["eligible"] is False
    assert "disabled by learning policy" in cheap["reasons"]
    assert cheap["observed_pulls"] == 12
