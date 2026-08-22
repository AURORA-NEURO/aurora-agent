from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousOfflineScenarioHarness,
    DomainEvaluatorRegistry,
    LLMRuntime,
    ModelCatalogue,
    BrainRunError,
)


class _ScenarioWorkspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        if name == "brain_model_select_contextual":
            raw_context = args.get("context")
            assert isinstance(raw_context, dict)
            context_identity = {
                field: raw_context.get(field)
                for field in ("domain", "capability", "risk_class", "task_family")
            }
            digest = hashlib.sha256(
                json.dumps(context_identity, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            ).hexdigest()
            return {
                "context_digest": digest,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "offline-model"},
                    "decision_digest": "d" * 64,
                    "ranking": [
                        {
                            "model_id": "offline/offline-model",
                            "eligible": True,
                            "reasons": [],
                            "base_score": 1.0,
                            "exploration_bonus": 0.0,
                            "score": 1.0,
                            "observed_pulls": 0,
                        }
                    ],
                },
            }
        if name == "brain_prompt_assemble":
            return {
                "messages": [
                    {"role": "system", "content": str(args.get("system"))},
                    {"role": "user", "content": str(args.get("task"))},
                ],
                "prompt_digest": "a" * 64,
            }
        if name == "brain_plan":
            return {
                "ok": True,
                "plan": {
                    "requires_approval": True,
                    "steps": [{"effect": "provider_call"}],
                    "plan_digest": "b" * 64,
                },
            }
        if name == "brain_outcome_record":
            state = json.loads(json.dumps(args["bandit_state"]))
            arm_id = str(args["arm_id"])
            arms = list(state.get("arms", []))
            existing = next((arm for arm in arms if arm.get("arm_id") == arm_id), None)
            if existing is None:
                arms.append({"arm_id": arm_id, "pulls": 1, "reward_sum": args["assessment"]["reward"], "failures": int(args["assessment"]["failed"])})
            else:
                existing["pulls"] += 1
                existing["reward_sum"] += args["assessment"]["reward"]
                existing["failures"] += int(args["assessment"]["failed"])
            state["arms"] = sorted(arms, key=lambda arm: arm["arm_id"])
            state["generation"] = int(state.get("generation", 0)) + 1
            return {"ok": True, "status": "recorded", "next_state": state}
        raise AssertionError(f"unexpected workspace tool: {name}")


def _agent() -> tuple[AutonomousAgent, list[str]]:
    calls: list[str] = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "offline",
        lambda request: calls.append(request.model) or {"output_text": "private provider response"},
    )
    candidate = {
        "provider": "offline",
        "model": "offline-model",
        "requires_credential": False,
        "capabilities": [
            "reasoning", "code", "science", "data", "web", "biomedical", "operations",
            "enterprise", "coordination", "multimodal", "evaluation",
        ],
        "context_window_tokens": 32_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 1,
        "cost_per_million_tokens": 0,
        "reliability": 0.99,
    }
    return AutonomousAgent(_ScenarioWorkspace(), runtime, model_catalogue=ModelCatalogue([candidate])), calls


def test_offline_scenario_matrix_covers_every_domain_and_replays_without_provider() -> None:
    agent, calls = _agent()
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    harness = AutonomousOfflineScenarioHarness(agent, evaluator_registry=registry)

    def evidence_for(context: dict[str, object]) -> dict[str, object]:
        preview = context["preview"]
        assert isinstance(preview, dict)
        domain = str(preview["domain"])
        profile = registry.resolve_for_autonomous_domain(domain).profile
        signals = {signal: 1.0 for signal in profile.required_signals}
        signals.update({signal: 1.0 for signal in profile.signal_weights})
        return {
            "domain": domain,
            "capability": "caller-review",
            "risk_class": "bounded-review",
            "signals": signals,
            "references": ["a" * 64],
            "limitations": ["caller-declared signals only"],
        }

    private_tasks = {domain: f"private task {domain} must not be retained" for domain in AUTONOMOUS_DOMAINS}
    report = harness.run_all(credentials={}, tasks=private_tasks, evidence_for=evidence_for)

    assert report["schema"] == "bioprism-autonomous-offline-scenario/0.1"
    assert report["status"] == "completed"
    assert report["case_count"] == len(AUTONOMOUS_DOMAINS)
    assert report["completed_count"] == len(AUTONOMOUS_DOMAINS)
    assert report["refused_count"] == 0
    assert {row["domain"] for row in report["cases"]} == set(AUTONOMOUS_DOMAINS)
    assert all(row["evaluation"]["passed"] for row in report["cases"])
    assert all(len(row["learning"]["outcome_digest"]) == 64 for row in report["cases"])
    assert calls == ["offline-model"] * len(AUTONOMOUS_DOMAINS)
    encoded = json.dumps(report)
    assert "private task" not in encoded
    assert "private provider response" not in encoded

    replay = harness.replay(report)
    assert replay["schema"] == "bioprism-autonomous-offline-scenario-replay/0.1"
    assert replay["verified_count"] == len(AUTONOMOUS_DOMAINS)
    assert replay["replayed_count"] == 0
    assert replay["idempotent"] is True
    assert calls == ["offline-model"] * len(AUTONOMOUS_DOMAINS)


def test_offline_scenario_replay_rejects_report_tampering() -> None:
    agent, _calls = _agent()
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    harness = AutonomousOfflineScenarioHarness(agent, evaluator_registry=registry)
    profile = registry.resolve_for_autonomous_domain("coding").profile
    report = harness.run(
        [{"id": "coding", "domain": "coding", "task": "transient coding scenario"}],
        credentials={},
        evidence_for=lambda _context: {
            "domain": "coding",
            "capability": "caller-review",
            "risk_class": "bounded-review",
            "signals": {signal: 1.0 for signal in set((*profile.required_signals, *profile.signal_weights))},
        },
    )
    tampered = json.loads(json.dumps(report))
    tampered["cases"][0]["evaluation"]["reward"] = 0.0
    with pytest.raises(BrainRunError, match="report digest"):
        harness.replay(tampered)
