import json

import pytest

from prism_sdk import (
    AutonomousBrain,
    BrainRunError,
    build_brain_evaluation_input,
    build_model_selection_audit,
    CredentialStore,
    LLMRuntime,
    openai_provider,
)


def _ranking() -> list[dict[str, object]]:
    return [
        {
            "model_id": "openai/strong",
            "eligible": True,
            "reasons": [],
            "base_score": 1.2,
            "exploration_bonus": 0.15,
            "score": 1.35,
            "observed_pulls": 8,
        },
        {
            "model_id": "anthropic/explore",
            "eligible": True,
            "reasons": [],
            "base_score": 1.0,
            "exploration_bonus": 0.3,
            "score": 1.3,
            "observed_pulls": 0,
        },
        {
            "model_id": "local/blocked",
            "eligible": False,
            "reasons": ["provider_circuit_open", "cost_limit_exceeded"],
            "base_score": 1.4,
            "exploration_bonus": 0.1,
            "score": 1.5,
            "observed_pulls": 21,
        },
    ]


def test_selection_audit_explains_gates_exploration_and_routing_stability_without_task_text():
    audit = build_model_selection_audit(
        {
            "task": "private task material must not be copied",
            "selection_status": "selected",
            "selected_model": {"provider": "openai", "model": "strong"},
            "decision_digest": "d" * 64,
            "selection_confidence": 0.019,
            "min_selection_confidence": 0.1,
            "ranking": _ranking(),
        }
    )

    assert audit["schema"] == "bioprism-brain-selection-audit/0.1"
    assert audit["selected_model"] == {
        "model_id": "openai/strong",
        "provider": "openai",
        "model": "strong",
    }
    assert audit["eligibility"] == {
        "eligible_count": 2,
        "rejected_count": 1,
        "rejection_counts": {"cost_limit_exceeded": 1, "provider_circuit_open": 1},
    }
    assert audit["exploration"]["unseen_eligible_count"] == 1  # type: ignore[index]
    assert audit["stability"]["runner_up_model_id"] == "anthropic/explore"  # type: ignore[index]
    assert 0 < audit["stability"]["routing_confidence"] < 1  # type: ignore[operator,index]
    assert audit["stability"]["kernel_selection_confidence"] == 0.019  # type: ignore[index]
    assert audit["stability"]["kernel_selection_confidence_floor"] == 0.1  # type: ignore[index]
    assert "private task material" not in json.dumps(audit)
    assert "transport success is not task reward" in audit["does_not_claim"]


def test_selection_audit_bounds_large_rankings_and_supports_refusal():
    ranking = []
    for index in range(70):
        ranking.append(
            {
                "model_id": f"provider/model-{index:02d}",
                "eligible": False,
                "reasons": ["provider_credential_unready"],
                "base_score": 0.0,
                "exploration_bonus": 0.0,
                "score": 0.0,
                "observed_pulls": 0,
            }
        )
    audit = build_model_selection_audit(
        {
            "selection_status": "refused_no_eligible_model",
            "selected_model": None,
            "ranking": ranking,
        }
    )

    assert len(audit["ranking"]) == 64
    assert audit["ranking_omitted"] == 6
    assert audit["selected_model"] is None
    assert audit["stability"]["routing_confidence"] == 0.0  # type: ignore[index]


def test_selection_audit_rejects_non_finite_ranking_scores():
    with pytest.raises(BrainRunError, match="must be finite"):
        build_model_selection_audit(
            {
                "selected_model": {"provider": "openai", "model": "bad"},
                "ranking": [
                    {
                        "model_id": "openai/bad",
                        "eligible": True,
                        "reasons": [],
                        "score": float("nan"),
                    }
                ],
            }
        )


def test_direct_brain_result_and_evaluator_input_carry_the_audit():
    class Workspace:
        def tool(self, name, arguments=None):
            if name == "brain_model_select":
                ranking = _ranking()[:2]
                ranking[0] = {**ranking[0], "model_id": "openai/test-model"}
                return {
                    "selection_status": "selected",
                    "selected_model": {"provider": "openai", "model": "test-model"},
                    "decision_digest": "a" * 64,
                    "ranking": ranking,
                }
            if name == "brain_prompt_assemble":
                return {"messages": [{"role": "user", "content": "bounded"}], "prompt_digest": "p" * 64}
            if name == "brain_plan":
                return {
                    "ok": True,
                    "plan": {
                        "requires_approval": True,
                        "steps": [{"effect": "provider_call"}],
                        "plan_digest": "q" * 64,
                    },
                }
            raise AssertionError(name)

    runtime = LLMRuntime(CredentialStore())
    runtime.register_provider(openai_provider(base_url="https://example.invalid"))
    result = AutonomousBrain(Workspace(), runtime).run(
        task="inspect",
        model_selection={},
        prompt={},
        plan={},
        credentials={},
    )

    assert result.status == "approval_required"
    audit = result.selection["selection_audit"]
    assert audit["selected_model"]["model_id"] == "openai/test-model"  # type: ignore[index]
    evaluator_input = build_brain_evaluation_input(result)
    assert evaluator_input["selection_audit"]["schema"] == "bioprism-brain-selection-audit/0.1"  # type: ignore[index]
