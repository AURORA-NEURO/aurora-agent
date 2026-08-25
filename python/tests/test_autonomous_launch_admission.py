from __future__ import annotations

import copy

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    BrainRunError,
    CredentialStore,
    InMemoryAutonomousRunTraceStore,
    LLMRuntime,
    ModelCatalogue,
    authorize_autonomous_launch_domains,
    create_autonomous_launch_admission,
    openai_provider,
    validate_autonomous_launch_admission,
)
from prism_sdk.autonomy import builtin_autonomous_workflow_strategies
from prism_sdk.domain_tools import builtin_autonomous_domain_tool_profiles
from prism_sdk.errors import ArgumentError


def _candidate() -> dict[str, object]:
    return {
        "provider": "openai",
        "model": "admission-model",
        "capabilities": ["reasoning", "code", "web", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
        "context_window_tokens": 32_000,
        "max_output_tokens": 2_000,
        "quality": 0.9,
        "latency_ms": 100,
        "cost_per_million_tokens": 10,
        "reliability": 0.95,
    }


def _agent() -> AutonomousAgent:
    runtime = LLMRuntime(CredentialStore())
    runtime.register_provider(openai_provider(base_url="https://launch-admission.invalid"))
    return AutonomousAgent(object(), runtime, model_catalogue=ModelCatalogue([_candidate()]))


def _complete_preflight(agent: AutonomousAgent) -> dict[str, object]:
    profiles = builtin_autonomous_workflow_strategies()
    tools = sorted({binding.name for profile in builtin_autonomous_domain_tool_profiles() for binding in profile.bindings})
    evidence = [
        f"{profile.domain}:{stage.id}:{output}"
        for profile in profiles
        for stage in profile.stages
        for output in stage.evidence_outputs
    ]
    capabilities = {
        name: {"configured": True, "operational": True, "restart_safe": True, "integrity_fenced": True, "caller_owned": True}
        for name in ("persistence", "queue", "approval_authority", "external_auth", "telemetry")
    }
    with agent.start_credential_session(session_id="launch-admission-ready") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        return agent.launch_preflight(
            available_tool_names=tools,
            available_evidence=evidence,
            deployment_capabilities=capabilities,
        )


def test_launch_admission_holds_blocked_preflight_across_all_domains() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    preflight = agent.launch_preflight()
    admission = agent.launch_admission(
        preflight,
        decision="approve",
        authorization_digest="a" * 64,
    )

    assert admission["schema"] == "bioprism-python-autonomous-launch-admission/0.1"
    assert admission["status"] == "held"
    assert admission["summary"]["domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert admission["summary"]["blocked_domain_count"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert all(row["admission_state"] == "blocked" for row in admission["domains"])
    assert validate_autonomous_launch_admission(admission) == admission


def test_launch_admission_approves_every_ready_domain_with_one_review_digest() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    admission = create_autonomous_launch_admission(
        preflight,
        decision="approve",
        authorization_digest="b" * 64,
        reason="reviewed launch gates",
    )

    assert admission["status"] == "approved"
    assert admission["summary"] == {
        "domain_count": 12,
        "selected_domain_count": 12,
        "approved_domain_count": 12,
        "held_domain_count": 0,
        "blocked_domain_count": 0,
        "not_selected_domain_count": 0,
    }
    assert all(row["admission_state"] == "approved" for row in admission["domains"])
    assert "reviewed launch gates" not in str(admission)
    assert "unit-test-only-not-a-provider-key" not in str(admission)
    assert validate_autonomous_launch_admission(admission) == admission


def test_launch_admission_supports_subset_and_hold_without_widening_authority() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    subset = agent.launch_admission(
        preflight,
        decision="approve",
        approved_domains=("coding",),
        authorization_digest="c" * 64,
    )
    assert subset["status"] == "approved"
    assert subset["summary"]["approved_domain_count"] == 1
    assert subset["summary"]["not_selected_domain_count"] == 11
    assert subset["domains"][0]["admission_state"] == "approved"
    assert all(row["admission_state"] == "not_selected" for row in subset["domains"][1:])

    held = create_autonomous_launch_admission(preflight, decision="hold", reason="wait for operator")
    assert held["status"] == "held"
    assert held["summary"]["held_domain_count"] == 12
    assert held["authorization_digest"] is None
    assert "wait for operator" not in str(held)


def test_launch_admission_rejects_missing_authority_and_tampering() -> None:
    agent = AutonomousAgent(None, LLMRuntime())
    preflight = agent.launch_preflight()
    with pytest.raises(ArgumentError, match="authorization_digest"):
        create_autonomous_launch_admission(preflight, decision="approve")
    tampered = copy.deepcopy(create_autonomous_launch_admission(preflight, decision="hold"))
    tampered["domains"][0]["admission_state"] = "approved"
    with pytest.raises(ArgumentError, match="admission_digest"):
        validate_autonomous_launch_admission(tampered)
    tampered["api_key"] = "must-not-cross"
    with pytest.raises(ArgumentError, match="secret-shaped"):
        validate_autonomous_launch_admission(tampered)


def test_launch_admission_gate_blocks_before_execution_and_checks_route_coverage() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    coding = agent.launch_admission(
        preflight,
        decision="approve",
        approved_domains=("coding",),
        authorization_digest="d" * 64,
    )
    authorize_autonomous_launch_domains(coding, ("coding",))
    with pytest.raises(ArgumentError, match="does not approve requested domains"):
        authorize_autonomous_launch_domains(coding, ("biomedical",))

    held = agent.launch_admission(preflight, decision="hold")
    with pytest.raises(ArgumentError, match="not approved"):
        agent.run_with_launch_admission(
            task="write a small function",
            domain="coding",
            launch_admission=held,
            credentials={},
            approve_provider_call=False,
        )

    with pytest.raises(ArgumentError, match="does not approve requested domains"):
        agent.run_auto_with_launch_admission(
            task="analyze a biomedical research result",
            launch_admission=coding,
            credentials={},
            approve_provider_call=False,
        )
    with pytest.raises(BrainRunError, match="requires provider-free routing"):
        agent.run_auto_with_launch_admission(
            task="write a small function",
            launch_admission=coding,
            credentials={},
            semantic_routing=True,
        )


def test_launch_admission_covers_explicit_cross_domain_and_resumable_batches_before_credentials() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    coding = agent.launch_admission(
        preflight,
        decision="approve",
        approved_domains=("coding",),
        authorization_digest="e" * 64,
    )
    with pytest.raises(ArgumentError, match="does not approve requested domains"):
        agent.run_batch_with_launch_admission(
            [{"task": "review the biomedical evidence", "domain": "biomedical"}],
            launch_admission=coding,
            credentials={},
        )
    with pytest.raises(ArgumentError, match="does not approve requested domains"):
        agent.run_cross_domain_batch_with_launch_admission(
            [{
                "task": "synthesize the reviewed findings",
                "subtasks": [
                    {"id": "coding", "task": "inspect the implementation", "domain": "coding"},
                    {"id": "biomedical", "task": "inspect the study", "domain": "biomedical"},
                ],
            }],
            launch_admission=coding,
            credentials={},
        )
    held = agent.launch_admission(preflight, decision="hold")
    with pytest.raises(ArgumentError, match="not approved"):
        agent.run_resumable_batch_with_launch_admission(
            [{"task": "review the implementation", "domain": "coding"}],
            job_id="held-batch",
            launch_admission=held,
            credentials={},
        )


def test_launch_admission_covers_capability_workflow_and_learning_facades_before_credentials() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    held = agent.launch_admission(preflight, decision="hold")
    blueprint = agent.prepare(task="review the implementation", domain="coding")
    common = {
        "blueprint": blueprint,
        "launch_admission": held,
        "credentials": {},
    }

    with pytest.raises(ArgumentError, match="not approved"):
        agent.run_capability_with_launch_admission(
            task="review the implementation",
            domain="coding",
            capability="implement",
            launch_admission=held,
            credentials={},
        )

    for method_name in (
        "run_workflow_with_launch_admission",
        "run_workflow_learning_with_launch_admission",
        "run_workflow_cycle_with_launch_admission",
        "run_workflow_trajectory_learning_with_launch_admission",
    ):
        with pytest.raises(ArgumentError, match="not approved"):
            getattr(agent, method_name)(**common)

    with pytest.raises(ArgumentError, match="not approved"):
        agent.run_workflow_with_trace_and_launch_admission(
            **common,
            trace_store=InMemoryAutonomousRunTraceStore(),
        )

    subtasks = [
        {"id": "coding", "task": "review the implementation", "domain": "coding"},
        {"id": "data", "task": "check the measurements", "domain": "data"},
    ]
    for method_name in (
        "run_cross_domain_learning_with_launch_admission",
        "run_cross_domain_trajectory_learning_with_launch_admission",
        "run_cross_domain_replan_learning_with_launch_admission",
    ):
        with pytest.raises(ArgumentError, match="not approved"):
            getattr(agent, method_name)(
                task="coordinate the review",
                subtasks=subtasks,
                launch_admission=held,
                credentials={},
            )


def test_launch_admission_rejects_semantic_routing_in_automatic_batches_before_dispatch() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    admission = agent.launch_admission(
        preflight,
        decision="approve",
        approved_domains=tuple(AUTONOMOUS_DOMAIN_NAMES),
        authorization_digest="f" * 64,
    )
    with pytest.raises(BrainRunError, match="requires provider-free routing"):
        agent.run_auto_batch_with_launch_admission(
            [{
                "task": "route this multidisciplinary review",
                "options": {"semantic_routing": True},
            }],
            launch_admission=admission,
            credentials={},
        )


def test_launch_admitted_batch_replays_an_options_factory_without_reinvoking_it() -> None:
    agent = _agent()
    preflight = _complete_preflight(agent)
    coding = agent.launch_admission(
        preflight,
        decision="approve",
        approved_domains=("coding",),
        authorization_digest="7" * 64,
    )
    calls: list[int] = []
    with agent.start_credential_session(session_id="launch-admitted-batch") as session:
        session.register_value("openai", "unit-test-only-not-a-provider-key")
        result = agent.run_batch_with_launch_admission(
            [{"task": "review the implementation", "domain": "coding"}],
            launch_admission=coding,
            credentials=session,
            options_factory=lambda _request, index: (
                calls.append(index) or {"approve_provider_call": False}
            ),
        )
    assert calls == [0]
    assert len(result.items) == 1
    assert result.items[0].status in {"failed", "refused"}
