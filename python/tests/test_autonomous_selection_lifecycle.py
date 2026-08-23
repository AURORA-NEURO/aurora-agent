from __future__ import annotations

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousSelectionPromotionLifecycle,
    AutonomousSelectionPromotionLifecycleStore,
    BrainRunError,
    LLMRuntime,
    ModelCatalogue,
    evaluate_autonomous_selection_policy,
    evaluate_autonomous_selection_promotion,
)


class _Workspace:
    def tool(self, _name: str, _arguments: dict) -> dict:
        raise AssertionError("provider or workspace tool must not be reached")


def _request(domain: str) -> dict:
    return {
        "task": f"private lifecycle task for {domain}",
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
        "provider_health": {"lab": {"provider": "lab", "circuit": "closed", "credential_required": False, "credential_ready": True, "structured_output_mode": "json_object"}},
        "model_health": {},
    }


def _cases(selected_arm_wins: bool) -> list[dict]:
    return [
        {
            "case_id": f"{domain}-lifecycle-case",
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


def test_selection_lifecycle_applies_hold_admission_rollback_and_restore() -> None:
    admitted = evaluate_autonomous_selection_promotion(
        evaluate_autonomous_selection_policy(_cases(True), require_all_domains=True)
    )
    held = evaluate_autonomous_selection_promotion(
        evaluate_autonomous_selection_policy(_cases(False), require_all_domains=True)
    )
    lifecycle = AutonomousSelectionPromotionLifecycle("selection-lifecycle-test", clock=lambda: 100)

    assert lifecycle.state.status == "uninitialized"
    assert lifecycle.apply(held).status == "held"
    assert lifecycle.apply(admitted).status == "admitted"
    assert lifecycle.state.generation == 1
    assert lifecycle.state.active_promotion_digest == admitted["promotion_digest"]

    store = AutonomousSelectionPromotionLifecycleStore()
    store.save(lifecycle.state)
    snapshot = store.snapshot()
    restored = AutonomousSelectionPromotionLifecycleStore()
    restored.restore(snapshot)
    assert restored.load().state_digest == lifecycle.state.state_digest

    rolled_back = lifecycle.rollback(reason="operator detected drift")
    assert rolled_back.status == "rolled_back"
    assert rolled_back.active_promotion_digest is None
    assert rolled_back.rollback_count == 1
    assert "private lifecycle task" not in str(rolled_back.to_dict())


def test_selection_lifecycle_joins_all_domain_readiness() -> None:
    promotion = evaluate_autonomous_selection_promotion(
        evaluate_autonomous_selection_policy(_cases(True), require_all_domains=True)
    )
    lifecycle = AutonomousSelectionPromotionLifecycle("selection-readiness-test", clock=lambda: 200)
    agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": ["structured_output", "reasoning", "science", "code", "web", "data", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_000,
                "quality": 0.9,
                "latency_ms": 10,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]),
        selection_promotion=lifecycle,
    )

    held = agent.readiness(selection_promotion_report=promotion, require_promoted_selection=True)
    assert held["domain_learning_coverage"]["selection_promotion"]["lifecycle_status"] == "uninitialized"
    assert held["domain_learning_coverage"]["selection_promotion"]["decision"] == "admit"
    assert len(held["domains"]) == len(AUTONOMOUS_DOMAINS)
    assert all(row["selection_promotion"]["domain_decision"] == "admit" for row in held["domains"])
    assert all(row["selection_promotion"]["status"] == "uninitialized" for row in held["domains"])
    with pytest.raises(BrainRunError, match="learned model selection is not admitted"):
        agent.prepare_auto_with_provider(
            task="route this browser research request",
            credentials={},
            model_candidates=agent.models(),
        )
    with pytest.raises(BrainRunError, match="learned model selection is not admitted"):
        agent.route_with_provider(
            task="route this browser research request",
            credentials={},
            model_candidates=agent.models(),
        )
    single_blueprint = agent.prepare_auto(task="fix this coding repository").blueprint
    cross_blueprint = agent.prepare_auto(
        task="write python code for the dataset pipeline",
        min_confidence=0.20,
        min_margin=0.10,
    ).cross_domain_blueprint
    assert single_blueprint is not None
    assert cross_blueprint is not None
    with pytest.raises(BrainRunError, match="learned model selection is not admitted"):
        agent.plan_with_provider(
            blueprint=single_blueprint,
            credentials={},
            model_candidates=agent.models(),
        )
    with pytest.raises(BrainRunError, match="learned model selection is not admitted"):
        agent.plan_cross_domain_with_provider(
            blueprint=cross_blueprint,
            credentials={},
            model_candidates=agent.models(),
        )

    agent.apply_selection_promotion(promotion)
    ready = agent.readiness(selection_promotion_report=promotion, require_promoted_selection=True)
    assert ready["domain_learning_coverage"]["selection_promotion"]["lifecycle_status"] == "admitted"
    assert ready["domain_learning_coverage"]["selection_promotion"]["active_promotion_digest"] == promotion["promotion_digest"]
    assert all(row["selection_promotion"]["status"] == "admitted" for row in ready["domains"])
    persisted = AutonomousSelectionPromotionLifecycleStore()
    snapshot = agent.save_selection_promotion(persisted)
    assert snapshot["state"]["active_promotion_digest"] == promotion["promotion_digest"]
    restored_lifecycle = AutonomousSelectionPromotionLifecycle("restored-selection-readiness-test", clock=lambda: 201)
    restored_agent = AutonomousAgent(
        _Workspace(),
        LLMRuntime(),
        model_catalogue=ModelCatalogue([
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": ["structured_output", "reasoning", "science", "code", "web", "data", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_000,
                "quality": 0.9,
                "latency_ms": 10,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]),
        selection_promotion=restored_lifecycle,
    )
    restored_state = restored_agent.restore_selection_promotion(persisted)
    assert restored_state["active_promotion_digest"] == promotion["promotion_digest"]
