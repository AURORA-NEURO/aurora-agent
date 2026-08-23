from __future__ import annotations

import hashlib
import json
from typing import Any, Mapping

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_AGENT_BATCH_SCHEMA,
    AutonomousAgent,
    AutonomousBatchResult,
    AutonomousBatchCheckpoint,
    AutonomousBrainBatchJobController,
    InMemoryAutonomousBatchCheckpointStore,
    TransactionalJsonAutonomousBatchCheckpointPersistence,
    BrainRunError,
    AutonomousDomainRegistry,
    InMemoryProvider,
    LLMRuntime,
    ModelCatalogue,
    ProviderError,
    ProviderModelDescriptor,
    ProviderOnboarding,
    ProviderRequest,
    ProviderStreamEvent,
    ProviderTool,
    ProviderToolResult,
)


class _CasTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_checkpoint_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["checkpoint_digest"]
        if observed != expected_checkpoint_digest:
            return False
        self.value = value
        return True


def _request(
    *,
    model: str = "offline-model",
    require_json: bool = False,
    response_schema: Mapping[str, Any] | None = None,
    tools: tuple[ProviderTool, ...] = (),
) -> ProviderRequest:
    return ProviderRequest(
        model=model,
        messages=({"role": "user", "content": "bounded local test"},),
        max_output_tokens=64,
        require_json=require_json,
        response_schema=response_schema,
        tools=tools,
    )


def test_in_memory_provider_is_explicit_credentialless_and_preserves_runtime_observations() -> None:
    seen: list[ProviderRequest] = []

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        seen.append(request)
        return {
            "model": request.model,
            "output_text": "offline answer",
            "request_id": "offline-request-1",
            "usage": {"input_tokens": 4, "output_tokens": 3},
            "raw_secret": "must never be retained",
        }

    runtime = LLMRuntime()
    config = runtime.register_in_memory_provider(
        "offline",
        handler,
        model_discovery_handler=lambda: {
            "data": [
                {
                    "id": "offline-model",
                    "context_window_tokens": 16_000,
                    "max_output_tokens": 2_048,
                    "capabilities": ["reasoning", "structured_output"],
                }
            ]
        },
    )

    assert isinstance(config.transport, InMemoryProvider)
    assert config.requires_credential is False
    assert config.to_metadata()["transport"] == "in_memory"
    onboarding = ProviderOnboarding(runtime)
    assert onboarding.status("offline")["ready"] is True
    assert onboarding.status("offline")["next_action"] == "ready"

    response = runtime.invoke("offline", _request())
    assert response.provider == "offline"
    assert response.model == "offline-model"
    assert response.text == "offline answer"
    assert response.raw == {
        "schema": "bioprism-llm-in-memory-provider/0.1",
        "transport": "caller_owned",
    }
    assert "raw_secret" not in json.dumps(response.to_dict())
    assert seen == [_request()]

    status = runtime.provider_status("offline")
    assert status["attempts"] == 1
    assert status["successes"] == 1
    assert status["credential_posture"] == "caller_supplied_in_memory_handle"
    assert runtime.model_status("offline", "offline-model")["success_rate"] == 1.0

    descriptors = runtime.discover_models("offline")
    assert descriptors == (
        ProviderModelDescriptor(
            "offline",
            "offline-model",
            capabilities=("reasoning", "structured_output"),
            context_window_tokens=16_000,
            max_output_tokens=2_048,
        ),
    )


def test_in_memory_provider_validates_structured_output_and_tool_intents() -> None:
    tools = (ProviderTool("read_status", parameters={"type": "object"}),)
    schema = {
        "type": "object",
        "required": ["answer"],
        "properties": {"answer": {"type": "string"}},
        "additionalProperties": False,
    }
    calls = 0

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        nonlocal calls
        calls += 1
        if request.tools and not any(
            message.get("role") == "tool" for message in request.messages
        ):
            return {
                "tool_calls": [
                    {
                        "call_id": "status-1",
                        "name": "read_status",
                        "arguments": {"scope": "workspace"},
                    }
                ]
            }
        return {"output_text": json.dumps({"answer": "ok"})}

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", handler)

    structured = runtime.invoke(
        "offline",
        _request(require_json=True, response_schema=schema),
    )
    assert structured.structured == {"answer": "ok"}

    result = runtime.invoke_tool_loop(
        "offline",
        _request(tools=tools),
        authorize_and_execute=lambda requested: [
            ProviderToolResult(
                call.call_id,
                {"status": "healthy"},
                approved=True,
            )
            for call in requested
        ],
    )
    assert result.status == "completed"
    assert result.tool_calls == 1
    assert result.final_response is not None
    assert calls == 3

    class BadNameProvider:
        def invoke(self, request: ProviderRequest) -> Mapping[str, Any]:
            return {"tool_calls": [{"call_id": "bad-1", "name": "write_file", "arguments": {}}]}

    bad_runtime = LLMRuntime()
    bad_runtime.register_in_memory_provider("bad", BadNameProvider().invoke)
    with pytest.raises(ProviderError, match="unrequested tool call"):
        bad_runtime.invoke("bad", _request(tools=tools))


def test_in_memory_provider_stream_handler_and_fallback_are_provider_neutral() -> None:
    stream_requests: list[ProviderRequest] = []

    def stream_handler(request: ProviderRequest) -> list[ProviderStreamEvent]:
        stream_requests.append(request)
        return [
            ProviderStreamEvent(
                provider="streaming",
                model=request.model,
                sequence=0,
                event_type="local.text",
                text_delta="hello",
            ),
            ProviderStreamEvent(
                provider="streaming",
                model=request.model,
                sequence=1,
                event_type="local.done",
                done=True,
            ),
        ]

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("streaming", lambda _request: "fallback", stream_handler=stream_handler)
    events = list(runtime.invoke_stream("streaming", _request()))
    assert [event.text_delta for event in events] == ["hello", ""]
    assert events[-1].done is True
    assert stream_requests == [_request()]

    fallback_runtime = LLMRuntime()
    fallback_runtime.register_in_memory_provider("fallback", lambda _request: "fallback text")
    fallback_events = list(fallback_runtime.invoke_stream("fallback", _request()))
    assert [event.text_delta for event in fallback_events] == ["fallback text", ""]
    assert fallback_events[-1].done is True


def test_in_memory_provider_redacts_handler_errors_while_preserving_retry_metadata() -> None:
    def handler(_request: ProviderRequest) -> str:
        raise ProviderError(
            "upstream api-key=local-secret must not escape",
            retryable=True,
            status_code=503,
        )

    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", handler)
    with pytest.raises(ProviderError) as raised:
        runtime.invoke("offline", _request())
    assert str(raised.value) == "in-memory provider handler failed"
    assert "local-secret" not in str(raised.value)
    assert raised.value.retryable is True
    assert raised.value.status_code == 503


class _OfflineWorkspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        args = {} if arguments is None else dict(arguments)
        if name == "brain_model_select_contextual":
            context = args.get("context")
            assert isinstance(context, dict)
            identity = {field: context.get(field) for field in ("domain", "capability", "risk_class", "task_family")}
            digest = hashlib.sha256(
                json.dumps(identity, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            ).hexdigest()
            return {
                "context_digest": digest,
                "selection_status": "selected",
                "selection": {
                    "selected_model": {"provider": "offline", "model": "offline-model"},
                    "decision_digest": "d" * 64,
                },
            }
        if name == "brain_model_select":
            return {
                "selected_model": {"provider": "offline", "model": "offline-model"},
                "decision_digest": "d" * 64,
            }
        if name == "brain_prompt_assemble":
            return {
                "messages": [
                    {"role": "system", "content": str(args.get("system"))},
                    {"role": "user", "content": str(args.get("task"))},
                ],
                "prompt_digest": "p" * 64,
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
            return {
                "ok": True,
                "status": "recorded",
                "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                "learning_evidence": {"evidence_digest": "e" * 64},
            }
        raise AssertionError(f"unexpected tool {name}")


def test_credentialless_runtime_executes_every_builtin_domain_through_agent_facade() -> None:
    runtime = LLMRuntime()
    requests: list[ProviderRequest] = []

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        requests.append(request)
        return {"output_text": f"offline result for {request.model}"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue(
        [
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]
    )
    agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=catalogue,
    )

    results = [
        agent.run(
            task=f"perform a bounded {domain} review",
            domain=domain,
            credentials={},
            approve_provider_call=True,
        )
        for domain in AUTONOMOUS_DOMAINS
    ]
    assert len(results) == len(AUTONOMOUS_DOMAINS)
    assert all(result.status == "completed_provider_call" for result in results)
    assert len(requests) == len(AUTONOMOUS_DOMAINS)
    assert {request.model for request in requests} == {"offline-model"}


def test_agent_run_batch_covers_every_domain_with_stable_metadata_and_opaque_credentials() -> None:
    runtime = LLMRuntime()

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        return {"output_text": f"offline result for {request.model}"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue(
        [
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]
    )
    agent = AutonomousAgent(_OfflineWorkspace(), runtime, model_catalogue=catalogue)
    requests = tuple(
        {"task": f"perform a bounded {domain} review", "domain": domain}
        for domain in AUTONOMOUS_DOMAINS
    )

    first = agent.run_batch(
        requests,
        credentials={},
        max_parallelism=3,
        options_factory=lambda _request, _index: {"approve_provider_call": True},
    )
    second = agent.run_batch(
        requests,
        credentials={},
        max_parallelism=3,
        options_factory=lambda _request, _index: {"approve_provider_call": True},
    )

    assert isinstance(first, AutonomousBatchResult)
    assert first.status == "completed"
    assert first.completed_count == len(AUTONOMOUS_DOMAINS)
    assert first.failed_count == 0
    assert first.omitted_count == 0
    assert [item.index for item in first.items] == list(range(len(AUTONOMOUS_DOMAINS)))
    assert all(item.status == "succeeded" for item in first.items)
    assert all(result is not None and result.status == "completed_provider_call" for result in first.results)
    assert first.batch_digest == second.batch_digest
    public = json.dumps(first.to_dict())
    assert AUTONOMOUS_AGENT_BATCH_SCHEMA in public
    assert "perform a bounded" not in public
    assert "offline result" not in public


def test_agent_run_batch_preflights_shape_and_accounts_for_stop_on_error_omissions() -> None:
    runtime = LLMRuntime()

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        return {"output_text": "offline answer"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue(
        [
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]
    )
    agent = AutonomousAgent(_OfflineWorkspace(), runtime, model_catalogue=catalogue)
    requests = (
        {"task": "complete a bounded coding review", "domain": "coding"},
        {"task": "reject this bounded data review", "domain": "data", "options": {"max_steps": 0}},
        {"task": "omit this later operations review", "domain": "operations"},
    )
    result = agent.run_batch(
        requests,
        credentials={},
        max_parallelism=1,
        stop_on_error=True,
        options_factory=lambda _request, _index: {"approve_provider_call": True},
    )
    assert result.status == "partial"
    assert result.completed_count == 1
    assert result.failed_count == 1
    assert result.omitted_count == 1
    assert [item.status for item in result.items] == ["succeeded", "failed", "omitted"]
    assert result.items[1].error_class == "BrainRunError"
    assert result.items[1].failure_code == "error"
    assert result.items[2].task_digest is None


def test_agent_run_auto_batch_routes_without_preselecting_domains_and_preserves_abstention() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider(
        "offline",
        lambda _request: {"output_text": "offline routed answer"},
    )
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=ModelCatalogue(
            [
                {
                    "provider": "offline",
                    "model": "offline-model",
                    "capabilities": sorted(required),
                    "context_window_tokens": 32_000,
                    "max_output_tokens": 2_048,
                    "quality": 0.9,
                    "latency_ms": 1,
                    "cost_per_million_tokens": 0,
                    "reliability": 0.99,
                }
            ]
        ),
    )
    result = agent.run_auto_batch(
        (
            {"task": "perform a bounded coding review"},
            {"task": "ask an entirely unclassified household question"},
        ),
        credentials={},
        max_parallelism=1,
        options_factory=lambda _request, _index: {"approve_provider_call": True},
    )
    assert result.status == "partial"
    assert result.completed_count == 1
    assert result.failed_count == 1
    assert result.items[0].result is not None
    assert result.items[0].result.status == "completed"
    assert result.items[0].result.route.primary_domain == "coding"
    assert result.items[1].status == "refused"
    assert result.items[1].result is not None
    assert result.items[1].result.status == "route_review_required"


def test_agent_run_cross_domain_batch_preserves_child_order_and_shared_approval_boundary() -> None:
    runtime = LLMRuntime()

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        return {"output_text": "offline cross-domain answer"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue(
        [
            {
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }
        ]
    )
    agent = AutonomousAgent(_OfflineWorkspace(), runtime, model_catalogue=catalogue)
    result = agent.run_cross_domain_batch(
        (
            {
                "task": "inspect coding and data readiness",
                "subtasks": (
                    {"id": "coding", "domain": "coding", "task": "inspect coding readiness"},
                    {"id": "data", "domain": "data", "task": "inspect data readiness"},
                ),
            },
        ),
        credentials={},
        max_parallelism=1,
        options_factory=lambda _request, _index: {
            "approve_provider_call": True,
            "synthesize": False,
        },
    )
    assert result.status == "completed"
    assert result.items[0].result is not None
    assert result.items[0].result.status == "children_completed"
    assert result.items[0].result.execution_child_ids == ("coding", "data")


def test_agent_run_resumable_batch_rehydrates_successes_and_rejects_tampering() -> None:
    runtime = LLMRuntime()
    calls: list[str] = []

    def handler(request: ProviderRequest) -> Mapping[str, Any]:
        calls.append(request.model)
        return {"output_text": "offline resumable answer"}

    runtime.register_in_memory_provider("offline", handler)
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=ModelCatalogue(
            [{
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }]
        ),
    )
    requests = (
        {"task": "complete a resumable coding review", "domain": "coding"},
        {"task": "complete a resumable data review", "domain": "data"},
    )
    fail_second = True
    checkpoints: list[AutonomousBatchCheckpoint] = []

    def options_factory(_request: Mapping[str, Any], index: int) -> Mapping[str, Any]:
        return {
            "approve_provider_call": True,
            "max_steps": 0 if fail_second and index == 1 else 32,
        }

    first = agent.run_resumable_batch(
        requests,
        job_id="restartable-batch",
        credentials={},
        max_parallelism=1,
        options_factory=options_factory,
        checkpoint_sink=checkpoints.append,
    )
    assert first.status == "partial"
    assert first.items[0].status == "succeeded"
    assert first.items[1].status == "failed"
    assert checkpoints[-1].completed_indices == (0,)
    public_checkpoint = json.dumps(checkpoints[-1].to_dict())
    assert "resumable coding review" not in public_checkpoint
    assert "offline resumable answer" not in public_checkpoint

    fail_second = False
    restored = agent.run_resumable_batch(
        requests,
        job_id="restartable-batch",
        credentials={},
        max_parallelism=1,
        options_factory=options_factory,
        checkpoint=checkpoints[-1].to_dict(),
        checkpoint_sink=checkpoints.append,
        rehydrate_result=lambda context: first.results[context.index],
    )
    assert restored.status == "completed"
    assert [item.status for item in restored.items] == ["succeeded", "succeeded"]
    assert len(calls) == 2
    assert checkpoints[-1].status == "completed"

    tampered = checkpoints[-1].to_dict()
    tampered["request_digests"][0] = "0" * 64
    with pytest.raises(BrainRunError, match="digest"):
        agent.run_resumable_batch(
            requests,
            job_id="restartable-batch",
            credentials={},
            max_parallelism=1,
            options_factory=options_factory,
            checkpoint=tampered,
            rehydrate_result=lambda context: restored.results[context.index],
        )


def test_autonomous_brain_batch_controller_owns_restore_persistence_restart_and_tamper_rejection() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", lambda _request: {"output_text": "offline controller answer"})
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=ModelCatalogue(
            [{
                "provider": "offline",
                "model": "offline-model",
                "capabilities": sorted(required),
                "context_window_tokens": 32_000,
                "max_output_tokens": 2_048,
                "quality": 0.9,
                "latency_ms": 1,
                "cost_per_million_tokens": 0,
                "reliability": 0.99,
            }]
        ),
    )
    requests = (
        {"task": "complete a controller coding review", "domain": "coding"},
        {"task": "complete a controller data review", "domain": "data"},
    )
    fail_second = True

    def options_factory(_request: Mapping[str, Any], index: int) -> Mapping[str, Any]:
        return {"approve_provider_call": True, "max_steps": 0 if fail_second and index == 1 else 32}

    store = TransactionalJsonAutonomousBatchCheckpointPersistence(_CasTextStore())
    controller = AutonomousBrainBatchJobController(agent, store)
    with pytest.raises(BrainRunError, match="restore"):
        controller.run(requests, job_id="python-controller-job", credentials={}, max_parallelism=1, options_factory=options_factory)
    assert controller.restore()["status"] == "empty"
    first = controller.run(
        requests,
        job_id="python-controller-job",
        credentials={},
        max_parallelism=1,
        stop_on_error=True,
        options_factory=options_factory,
    )
    assert first["batch"].status == "partial"
    assert [item.status for item in first["batch"].items] == ["succeeded", "failed"]
    persisted = store.read()
    assert persisted is not None
    assert first["controller"]["completed_items"] == 1
    encoded = json.dumps(persisted, sort_keys=True)
    assert "controller coding review" not in encoded
    assert "offline controller answer" not in encoded

    fail_second = False
    restarted = AutonomousBrainBatchJobController(agent, store)
    assert restarted.restore()["status"] == "restored"
    completed = restarted.run(
        requests,
        job_id="python-controller-job",
        credentials={},
        max_parallelism=1,
        stop_on_error=True,
        options_factory=options_factory,
        rehydrate_result=lambda context: first["batch"].items[context.index].result,
    )
    assert completed["batch"].status == "completed"
    assert [item.status for item in completed["batch"].items] == ["succeeded", "succeeded"]
    assert completed["controller"]["status"] == "completed"
    assert store.read()["status"] == "completed"  # type: ignore[index]

    tampered = dict(store.read() or {})
    tampered["request_digests"] = ["0" * 64, *tampered["request_digests"][1:]]
    tampered_store = type(
        "TamperedStore",
        (),
        {"read": lambda _self: tampered, "write": lambda _self, _value: None},
    )()
    invalid = AutonomousBrainBatchJobController(agent, tampered_store)
    with pytest.raises(BrainRunError, match="digest|checkpoint"):
        invalid.restore()


def test_batch_checkpoint_json_cas_is_bounded_and_tamper_evident_across_all_domains() -> None:
    request_digests = tuple(hashlib.sha256(f"batch-{domain}".encode("utf-8")).hexdigest() for domain in AUTONOMOUS_DOMAINS)
    checkpoint = AutonomousBatchCheckpoint(
        job_id="all-domains-batch",
        mode="domain",
        batch_input_digest=hashlib.sha256(b"all-domain-batch-input").hexdigest(),
        request_digests=request_digests,
        completed_indices=tuple(range(0, len(request_digests), 2)),
        completed_result_digests=tuple(hashlib.sha256(f"result-{index}".encode("utf-8")).hexdigest() for index in range(0, len(request_digests), 2)),
        max_parallelism=4,
        stop_on_error=True,
        status="partial",
    )
    backend = _CasTextStore()
    persistence = TransactionalJsonAutonomousBatchCheckpointPersistence(backend)
    assert persistence.write_if_unchanged(None, checkpoint)
    restored = persistence.read()
    assert restored is not None
    assert restored["checkpoint_digest"] == checkpoint.checkpoint_digest
    assert len(restored["request_digests"]) == len(AUTONOMOUS_DOMAINS)
    assert not persistence.write_if_unchanged(None, checkpoint)
    tampered = json.loads(backend.value)
    tampered["completed_indices"] = list(reversed(tampered["completed_indices"]))
    backend.value = json.dumps(tampered)
    with pytest.raises(BrainRunError, match="checkpoint|digest"):
        persistence.read()


def test_agent_run_resumable_batch_supports_route_first_and_cross_domain_modes() -> None:
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("offline", lambda _request: {"output_text": "offline route"})
    required = {
        capability
        for profile in AutonomousDomainRegistry.with_builtin_profiles().catalogue()
        for capability in profile["required_model_capabilities"]
    }
    required.update({"tool_calling", "structured_output"})
    catalogue = ModelCatalogue([{
        "provider": "offline",
        "model": "offline-model",
        "capabilities": sorted(required),
        "context_window_tokens": 32_000,
        "max_output_tokens": 2_048,
        "quality": 0.9,
        "latency_ms": 1,
        "cost_per_million_tokens": 0,
        "reliability": 0.99,
    }])
    agent = AutonomousAgent(_OfflineWorkspace(), runtime, model_catalogue=catalogue)
    options = lambda _request, _index: {"approve_provider_call": True}

    auto_checkpoints: list[AutonomousBatchCheckpoint] = []
    auto = agent.run_resumable_batch(
        ({"task": "perform a bounded coding review"},),
        job_id="route-first-batch",
        mode="auto",
        credentials={},
        max_parallelism=1,
        options_factory=options,
        checkpoint_sink=auto_checkpoints.append,
    )
    assert auto.status == "completed"
    assert auto.items[0].result is not None
    assert auto.items[0].result.status == "completed"
    assert auto_checkpoints[-1].mode == "auto"

    cross_checkpoints: list[AutonomousBatchCheckpoint] = []
    cross = agent.run_resumable_batch(
        ({
            "task": "inspect coding and data readiness",
            "subtasks": (
                {"id": "coding", "domain": "coding", "task": "inspect coding readiness"},
                {"id": "data", "domain": "data", "task": "inspect data readiness"},
            ),
        },),
        job_id="cross-domain-batch",
        mode="cross_domain",
        credentials={},
        max_parallelism=1,
        options_factory=lambda _request, _index: {"approve_provider_call": True, "synthesize": False},
        checkpoint_sink=cross_checkpoints.append,
    )
    assert cross.status == "completed"
    assert cross.items[0].result is not None
    assert cross.items[0].result.status == "children_completed"
    assert cross_checkpoints[-1].mode == "cross_domain"
