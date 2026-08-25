from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    AUTONOMOUS_DOMAINS,
    BrainRunError,
    DomainEvaluatorRegistry,
    create_autonomous_cycle_evaluator_bridge,
)


def _input(domain: str, *, evidence: object = None) -> dict[str, object]:
    return {
        "schema": "bioprism-brain-evaluator-input/0.1",
        "run_id": f"run-{domain}",
        "status": "completed_provider_call",
        "result_kind": "run",
        "outcome_digest": "a" * 64,
        "learning_outcome_digest": "b" * 64,
        "context_digest": "c" * 64,
        "context": {
            "domain": domain,
            "capability": "caller_review",
            "risk_class": "read_only",
        },
        "task": "private task text must never cross the bridge",
        "evidence": evidence,
    }


def test_bridge_covers_every_builtin_domain_and_exposes_only_metadata() -> None:
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    contexts: list[dict[str, object]] = []

    def evidence_for(context: dict[str, object]) -> dict[str, object]:
        contexts.append(dict(context))
        domain = str(context["domain"])
        profile = registry.resolve_for_autonomous_domain(domain).profile
        return {
            "domain": domain,
            "capability": "caller_review",
            "risk_class": "read_only",
            "signals": {
                signal: 1.0
                for signal in (*profile.required_signals, *profile.signal_weights)
            },
            "references": ["d" * 64],
        }

    bridge = create_autonomous_cycle_evaluator_bridge(evidence_for, evaluator_registry=registry)
    assert bridge.schema == AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA
    assert bridge.to_dict()["domain_count"] == len(AUTONOMOUS_DOMAINS)
    assert len(bridge.evaluator_catalogue_digest) == 64
    assert len(bridge.policy_digest) == 64

    for domain in AUTONOMOUS_DOMAINS:
        if domain == "cross_domain":
            continue
        decision = bridge.evaluator_for_domain(domain).assess_value_only_input(_input(domain))
        assert decision.passed is True
        assert decision.evaluator_id == registry.resolve_for_autonomous_domain(domain).evaluator_id

    assert len(contexts) == len(AUTONOMOUS_DOMAINS) - 1
    for context in contexts:
        assert context["schema"] == AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA
        assert context["secret_material"] == "never_returned"
        assert "task" not in context
        assert "evidence" not in context
        assert "prompt" not in context
        assert "credentials" not in context
        assert "private task text" not in json.dumps(context)


def test_cross_domain_bridge_routes_specialists_and_synthesis_to_exact_profiles() -> None:
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    seen: list[dict[str, object]] = []

    def evidence_for(context: dict[str, object]) -> dict[str, object]:
        seen.append(dict(context))
        domain = str(context["domain"])
        profile = registry.resolve_for_autonomous_domain(domain).profile
        return {
            "domain": domain,
            "capability": "cross_domain_review",
            "risk_class": "read_only",
            "signals": {signal: 1.0 for signal in profile.required_signals},
        }

    bridge = create_autonomous_cycle_evaluator_bridge(evidence_for, evaluator_registry=registry)
    evaluator = bridge.evaluator_for_cross_domain(("coding", "data"))
    coding = evaluator.assess_value_only_input(_input("coding"))
    data = evaluator.assess_value_only_input(_input("data"))
    synthesis = evaluator.assess_value_only_input(_input("cross_domain"))

    assert coding.passed and data.passed and synthesis.passed
    assert coding.evaluator_id == "autonomous-cycle-cross-domain-quality"
    assert [context["role"] for context in seen] == ["specialist", "specialist", "synthesis"]
    assert [context["domain"] for context in seen] == ["coding", "data", "cross_domain"]
    assert all(context["selected_domains"] == ["coding", "data"] for context in seen)
    assert all(context["mode"] == "cross_domain" for context in seen)


def test_bridge_fails_closed_for_inline_values_missing_coverage_and_bad_factory() -> None:
    bridge = create_autonomous_cycle_evaluator_bridge(lambda _context: {})
    with pytest.raises(BrainRunError, match="inline evidence"):
        bridge.evaluator_for_domain("coding").assess_value_only_input(
            _input("coding", evidence={"domain": "coding"})
        )

    with pytest.raises(BrainRunError, match="evaluator callback failed"):
        bridge.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))

    incomplete = DomainEvaluatorRegistry.with_builtin_profiles()
    with pytest.raises(BrainRunError, match="no evaluator is registered"):
        create_autonomous_cycle_evaluator_bridge(lambda _context: {}, evaluator_registry=incomplete)

    malformed = create_autonomous_cycle_evaluator_bridge(lambda _context: None)  # type: ignore[arg-type]
    with pytest.raises(BrainRunError, match="must return a mapping"):
        malformed.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))


def test_bridge_preserves_explicit_failure_and_replan_without_provider_success_inference() -> None:
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()

    def evidence_for(context: dict[str, object]) -> dict[str, object]:
        profile = registry.resolve_for_autonomous_domain(str(context["domain"])).profile
        return {
            "domain": context["domain"],
            "capability": "caller_review",
            "risk_class": "read_only",
            "signals": {signal: 0.0 for signal in profile.required_signals},
        }

    decision = create_autonomous_cycle_evaluator_bridge(
        evidence_for,
        evaluator_registry=registry,
    ).evaluator_for_domain("coding").assess_value_only_input(_input("coding"))
    assert decision.passed is False
    assert decision.failed is True
    assert decision.reward == 0.0
    assert decision.replan_requested is True
    assert decision.failure_class == "domain_evidence_gate"
