from __future__ import annotations

import json
import re

import pytest

from prism_sdk import (
    AUTONOMOUS_CYCLE_EVALUATOR_BRIDGE_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AutonomousEvidenceSourceReceipt,
    BrainRunError,
    calibrate_autonomous_evaluators,
    content_digest,
    DomainEvaluatorRegistry,
    create_autonomous_cycle_evaluator_bridge,
)

_DIGEST = re.compile(r"^[0-9a-f]{64}$")


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


def _source_receipt(domain: str, **overrides: object) -> dict[str, object]:
    descriptor: dict[str, object] = {
        "request_digest": "a" * 64,
        "plan_digest": "b" * 64,
        "requirement_id": f"{domain}:source:observe",
        "domain": domain,
        "source_id": f"fixture-source-{domain}",
        "source_digest": content_digest({"domain": domain, "revision": 1}),
        "value_digest": content_digest({"domain": domain, "value": "transient"}),
        "value_bytes": 32,
        "provider": "fixture-provider",
        "protocol": "http_json",
        "adapter_id": "fixture-source-adapter",
        "contract_digest": "c" * 64,
        "policy_digest": "d" * 64,
        "source_kind": "json",
        "freshness": "realtime",
        "authority": "provider_observed",
        "status": "observed",
        "observed_at_ms": 1,
        "expires_at_ms": None,
        "citation_digest": content_digest({"domain": domain, "citation": "fixture"}),
        "decision": "accepted",
        "decision_reasons": (),
        "limitations": ("Caller-owned offline fixture.",),
    }
    descriptor.update(overrides)
    receipt_descriptor = {
        "schema": "bioprism-python-autonomous-evidence-source/0.1",
        **descriptor,
        "retention": "metadata_only;raw_source_values_and_locators_caller_owned",
        "secret_material": "never_returned",
    }
    receipt = AutonomousEvidenceSourceReceipt(
        request_digest=descriptor["request_digest"],
        plan_digest=descriptor["plan_digest"],
        requirement_id=descriptor["requirement_id"],
        domain=descriptor["domain"],
        source_id=descriptor["source_id"],
        source_digest=descriptor["source_digest"],
        value_digest=descriptor["value_digest"],
        value_bytes=descriptor["value_bytes"],
        provider=descriptor["provider"],
        protocol=descriptor["protocol"],
        adapter_id=descriptor["adapter_id"],
        contract_digest=descriptor["contract_digest"],
        policy_digest=descriptor["policy_digest"],
        source_kind=descriptor["source_kind"],
        freshness=descriptor["freshness"],
        authority=descriptor["authority"],
        status=descriptor["status"],
        observed_at_ms=descriptor["observed_at_ms"],
        expires_at_ms=descriptor["expires_at_ms"],
        citation_digest=descriptor["citation_digest"],
        decision=descriptor["decision"],
        decision_reasons=tuple(descriptor["decision_reasons"]),
        limitations=tuple(descriptor["limitations"]),
        receipt_digest=content_digest(receipt_descriptor),
    )
    return receipt.to_dict()


def _calibration_report(registry: DomainEvaluatorRegistry, domain: str = "coding", *, complete: bool = True) -> dict[str, object]:
    profile = registry.resolve_for_autonomous_domain(domain).profile
    signals = {signal: 1.0 for signal in (profile.required_signals if complete else ("schema_valid",))}

    def case(case_id: str, split: str) -> dict[str, object]:
        return {
            "case_id": case_id,
            "domain": domain,
            "split": split,
            "label": 1 if complete else 0,
            "evidence": {
                "schema": "bioprism-brain-domain-evaluator/0.1",
                "domain": domain,
                "capability": "fixture_review",
                "risk_class": "read_only",
                "signals": signals,
                "references": [],
                "limitations": ["Caller-owned offline fixture."],
                "stage_plan_digest": None,
                "capability_contract_digests": [],
                "selected_tool_names": [],
                "retention": "value_only_digests_and_signal_scores",
            },
        }

    return calibrate_autonomous_evaluators(
        [case(f"{domain}-calibration", "calibration"), case(f"{domain}-holdout", "holdout")],
        registry=registry,
        domains=[domain],
        min_calibration_cases_per_domain=1,
        min_holdout_cases_per_domain=1,
        require_all_domains=False,
    )


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


def test_bridge_gates_learning_on_accepted_source_and_calibrated_evaluator_receipts() -> None:
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    contexts: list[dict[str, object]] = []
    report = _calibration_report(registry)

    def evidence_for(context: dict[str, object]) -> dict[str, object]:
        contexts.append(dict(context))
        domain = str(context["domain"])
        profile = registry.resolve_for_autonomous_domain(domain).profile
        return {
            "domain": domain,
            "capability": "fixture_review",
            "risk_class": "read_only",
            "signals": {signal: 1.0 for signal in profile.required_signals},
        }

    bridge = create_autonomous_cycle_evaluator_bridge(
        evidence_for,
        evaluator_registry=registry,
        source_receipt_for=lambda context: _source_receipt(str(context["domain"])),
        evaluator_calibration_for=lambda _context: report,
    )
    decision = bridge.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))
    assert decision.passed is True
    assert contexts[0]["source_decision"] == "accepted"
    assert contexts[0]["source_authority"] == "provider_observed"
    assert contexts[0]["evaluator_calibration_decision"] == "admit_learning"
    assert _DIGEST.fullmatch(str(contexts[0]["source_receipt_digest"]))
    assert _DIGEST.fullmatch(str(contexts[0]["evaluator_calibration_digest"]))


def test_bridge_refuses_stale_or_missing_authority_before_evidence_callback() -> None:
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    evidence_calls = 0

    def evidence_for(_context: dict[str, object]) -> dict[str, object]:
        nonlocal evidence_calls
        evidence_calls += 1
        return {}

    rejected_source = create_autonomous_cycle_evaluator_bridge(
        evidence_for,
        evaluator_registry=registry,
        source_receipt_for=lambda context: _source_receipt(
            str(context["domain"]), authority="caller_declared", decision="unverified"
        ),
    )
    with pytest.raises(BrainRunError, match="accepted authoritative observation"):
        rejected_source.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))
    assert evidence_calls == 0

    missing_calibration = create_autonomous_cycle_evaluator_bridge(
        evidence_for,
        evaluator_registry=registry,
        evaluator_calibration_for=lambda _context: None,
    )
    with pytest.raises(BrainRunError, match="calibration report is required"):
        missing_calibration.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))
    assert evidence_calls == 0

    held_calibration = create_autonomous_cycle_evaluator_bridge(
        evidence_for,
        evaluator_registry=registry,
        evaluator_calibration_for=lambda _context: _calibration_report(
            registry, complete=False
        ),
    )
    with pytest.raises(BrainRunError, match="calibration holds"):
        held_calibration.evaluator_for_domain("coding").assess_value_only_input(_input("coding"))
    assert evidence_calls == 0
