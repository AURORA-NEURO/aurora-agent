"""Operator-facing command line boundary for the autonomous SDK.

The SDK is intentionally embeddable, but a useful autonomous system also needs a small,
well-defined process boundary.  This module provides that boundary without moving secrets into
the brain or into MCP:

* ``catalogue`` and ``evidence-plan`` are provider-free inspection commands;
* ``route`` exposes deterministic routing evidence without invoking a model;
* ``provider-status``, ``onboard``, and the inventory commands implement the redacted BYOK and
  model-lifecycle boundaries; and
* ``state-status``, ``learning-status``, ``execution-status``, and ``workflow-status`` inspect
  persisted health, learning, execution, and staged workflow metadata, while
  ``settle-learning`` accepts only a bounded evaluator decision for a restart-safe settlement; and
* ``run`` connects to a caller-owned MCP workspace, collects one short-lived credential, lets the
  existing autonomous planner select a model, and requires explicit provider/mission approval; and
* ``batch-run`` applies the same model, provider, approval, learning, and execution boundaries to
  a bounded request file spanning explicit-domain, automatic, or cross-domain work.

The command line parser deliberately has no API-key, token, header, or secret argument.  Keys
are accepted only through the existing no-echo prompt or an explicitly named environment
variable, then revoked when the command exits.  MCP commands are passed as argv after parsing;
no shell is ever started by this boundary.
"""

from __future__ import annotations

import argparse
import getpass
import json
import os
import shlex
import sys
import tempfile
import threading
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence, TextIO

from .authoring import content_digest
from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousBatchItem,
    AutonomousBatchCheckpoint,
    AutonomousBatchResult,
    AutonomousBrainBatchJobController,
    AutonomousWorkflowCheckpoint,
    InMemoryAutonomousBatchCheckpointStore,
)
from .autonomous_model_inventory import AutonomousModelInventoryStore
from .autonomy_persistence import AutonomousExecutionJournal
from .brain import BrainEvaluatorDecision, BrainLearningEpisode, BrainOutcomeEvaluator
from .brain_learning_store import SQLiteBrainLearningLedger
from .client import Client
from .errors import SdkError
from .evaluators import builtin_autonomous_domain_evaluator_profiles
from .llm_runtime import (
    LLMRuntime,
    ModelCandidate,
    ModelCatalogue,
    ProviderHealthLedger,
    ProviderModelDescriptor,
    ProviderOnboarding,
    ProviderTool,
    ProviderToolCall,
    ProviderToolResult,
    anthropic_provider,
    openai_compatible_provider,
    openai_provider,
)


CLI_SCHEMA = "aurora-autonomous-cli/0.1"
WORKFLOW_CHECKPOINT_STORE_SCHEMA = "aurora-autonomous-workflow-checkpoint-store/0.1"
BATCH_CHECKPOINT_STORE_SCHEMA = "aurora-autonomous-batch-checkpoint-store/0.1"
BATCH_RESULT_MANIFEST_SCHEMA = "aurora-autonomous-batch-result-manifest/0.1"
AUTONOMOUS_BATCH_REQUESTS_SCHEMA = "aurora-autonomous-batch-requests/0.1"
_DEFAULT_CONTEXT_WINDOW = 128_000
_DEFAULT_MAX_OUTPUT = 4_096
_DEFAULT_TIMEOUT = 30.0
_LOCAL_PROVIDER_NAMES = frozenset({"local", "in_memory"})
_MAX_WORKFLOW_CHECKPOINT_STORE_BYTES = 1_000_000
_MAX_BATCH_REQUEST_FILE_BYTES = 4_000_000
_MAX_BATCH_RESULT_MANIFEST_BYTES = 1_000_000
_MAX_LOCAL_RESPONSE_SEQUENCE = 32
_MAX_MCP_PROVIDER_TOOLS = 128


class _CliArgumentError(ValueError):
    """Internal parse failure whose message is intentionally not echoed."""


class _ArgumentParser(argparse.ArgumentParser):
    """Argparse shell that does not print potentially sensitive argv on failures."""

    def error(self, _message: str) -> None:
        raise _CliArgumentError("invalid command-line arguments")

    def exit(self, status: int = 0, _message: str | None = None) -> None:
        if status:
            raise _CliArgumentError("invalid command-line arguments")
        raise SystemExit(status)


class _OfflineWorkspace:
    """Workspace used by provider-free commands to prevent accidental execution."""

    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        raise RuntimeError(f"provider-free command attempted to call workspace tool {name!r}")


class _McpWorkspace:
    """Adapt the synchronous MCP client to the brain's narrow workspace protocol."""

    def __init__(self, client: Client) -> None:
        self.client = client

    def tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> dict[str, Any]:
        return self.client.call_tool(name, arguments).require_ok()


def _json_default(value: Any) -> Any:
    """Project only known SDK value objects; never stringify arbitrary runtime state."""

    to_dict = getattr(value, "to_dict", None)
    if callable(to_dict):
        projected = to_dict()
        if isinstance(projected, Mapping):
            return dict(projected)
    raise TypeError(f"value of type {type(value).__name__} is not CLI-projectable")


def _write_json(writer: TextIO, value: Any) -> None:
    writer.write(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            indent=2,
            allow_nan=False,
            default=_json_default,
        )
    )
    writer.write("\n")


def _domains(values: Sequence[str] | None) -> tuple[str, ...]:
    selected = tuple(AUTONOMOUS_DOMAINS if not values else values)
    if not selected:
        raise ValueError("at least one domain is required")
    unknown = sorted(set(selected).difference(AUTONOMOUS_DOMAINS))
    if unknown:
        raise ValueError("unsupported autonomous domain(s): " + ", ".join(unknown))
    if len(set(selected)) != len(selected):
        raise ValueError("domains must be unique")
    return selected


def _provider_config(args: argparse.Namespace):
    provider = args.provider
    base_url = args.base_url
    if provider == "openai":
        return openai_provider(base_url=base_url or "https://api.openai.com")
    if provider == "anthropic":
        return anthropic_provider(base_url=base_url or "https://api.anthropic.com")
    if not base_url:
        raise ValueError("--base-url is required for a custom OpenAI-compatible provider")
    return openai_compatible_provider(
        provider,
        base_url,
        path=args.provider_path,
        models_path=args.models_path,
    )


def _runtime_with_provider(args: argparse.Namespace) -> tuple[LLMRuntime, ProviderOnboarding]:
    runtime = LLMRuntime()
    onboarding = ProviderOnboarding(runtime)
    if args.provider in _LOCAL_PROVIDER_NAMES:
        local_response = args.local_response
        local_response_json = args.local_response_json
        local_response_sequence_json = args.local_response_sequence_json
        if local_response_json is not None and local_response_sequence_json is not None:
            raise ValueError(
                "--local-response-json and --local-response-sequence-json cannot be combined"
            )
        if local_response_json is not None:
            try:
                parsed_response = json.loads(local_response_json)
            except (TypeError, json.JSONDecodeError) as error:
                raise ValueError("--local-response-json must be valid JSON") from error
            if not isinstance(parsed_response, Mapping):
                raise ValueError("--local-response-json must be a JSON object")
        else:
            parsed_response = None
        if local_response_sequence_json is not None:
            try:
                parsed_sequence = json.loads(local_response_sequence_json)
            except (TypeError, json.JSONDecodeError) as error:
                raise ValueError("--local-response-sequence-json must be valid JSON") from error
            if (
                not isinstance(parsed_sequence, list)
                or not 1 <= len(parsed_sequence) <= _MAX_LOCAL_RESPONSE_SEQUENCE
                or any(not isinstance(item, Mapping) for item in parsed_sequence)
            ):
                raise ValueError(
                    "--local-response-sequence-json must be an array of 1 to "
                    f"{_MAX_LOCAL_RESPONSE_SEQUENCE} JSON objects"
                )
            response_sequence = tuple(dict(item) for item in parsed_sequence)
        else:
            response_sequence = ()
        sequence_lock = threading.Lock()
        sequence_index = 0

        def local_handler(_request: Any) -> Mapping[str, Any]:
            nonlocal sequence_index
            if response_sequence:
                with sequence_lock:
                    if sequence_index >= len(response_sequence):
                        raise ValueError("local provider response sequence is exhausted")
                    sequence_item = response_sequence[sequence_index]
                    sequence_index += 1
                sequence_text = sequence_item.get("text", sequence_item.get("output_text"))
                if not isinstance(sequence_text, str):
                    sequence_text = json.dumps(
                        sequence_item,
                        ensure_ascii=False,
                        separators=(",", ":"),
                    )
                return {
                    "text": sequence_text,
                    "structured": dict(sequence_item),
                    "tool_calls": list(sequence_item.get("tool_calls", ())),
                    "request_id": f"local-cli-sequence-{sequence_index}",
                }
            if parsed_response is not None:
                return {
                    "text": json.dumps(parsed_response, ensure_ascii=False, separators=(",", ":")),
                    "structured": dict(parsed_response),
                    "request_id": "local-cli-request",
                }
            return {
                "text": local_response,
                "request_id": "local-cli-request",
            }

        local_model = args.local_model
        onboarding.register_provider(
            runtime.register_in_memory_provider(
                args.provider,
                local_handler,
                model_discovery_handler=lambda: {
                    "data": [{
                        "id": local_model,
                        "owned_by": "aurora-local",
                    }]
                },
            )
        )
    else:
        onboarding.register_provider(_provider_config(args))
    return runtime, onboarding


def _mcp_provider_tools(client: Client) -> tuple[ProviderTool, ...]:
    """Convert the live MCP catalogue into a bounded model-visible tool surface."""

    schemas = client.list_tools()
    if len(schemas) > _MAX_MCP_PROVIDER_TOOLS:
        raise ValueError(
            "MCP workspace advertises more than "
            f"{_MAX_MCP_PROVIDER_TOOLS} model-visible tools"
        )
    tools: list[ProviderTool] = []
    seen: set[str] = set()
    for schema in schemas:
        tool = ProviderTool.from_mcp_schema(schema)
        if tool.name in seen:
            raise ValueError("MCP workspace advertises duplicate tool names")
        seen.add(tool.name)
        tools.append(tool)
    return tuple(tools)


def _cli_tool_authorizer(
    client: Client,
    *,
    approve_mission_dispatch: bool,
) -> Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]]:
    """Build the CLI's explicit approval boundary for provider-requested MCP calls."""

    def authorize(calls: tuple[ProviderToolCall, ...]) -> Sequence[ProviderToolResult]:
        if not approve_mission_dispatch:
            return tuple(
                ProviderToolResult(
                    call.call_id,
                    {"ok": False, "status": "approval_required", "authorization": "operator"},
                    approved=False,
                    is_error=True,
                )
                for call in calls
            )
        results: list[ProviderToolResult] = []
        for call in calls:
            try:
                value = client.call_tool(call.name, call.arguments).require_ok()
            except Exception:
                return tuple(
                    ProviderToolResult(
                        pending.call_id,
                        {"ok": False, "status": "tool_execution_failed"},
                        approved=False,
                        is_error=True,
                    )
                    for pending in calls
                )
            results.append(ProviderToolResult(call.call_id, value, approved=True))
        return tuple(results)

    return authorize


def _candidate_args(
    args: argparse.Namespace,
    descriptors: Sequence[ProviderModelDescriptor] = (),
) -> tuple[ModelCandidate, ...]:
    models = tuple(args.model or ())
    capabilities = tuple(args.model_capability or ())
    if descriptors:
        selected = tuple(
            descriptor
            for descriptor in descriptors
            if not models or descriptor.model in models
        )
        if models:
            discovered_names = {descriptor.model for descriptor in descriptors}
            missing = tuple(model for model in models if model not in discovered_names)
            if missing:
                raise ValueError("requested model was not returned by provider discovery")
        candidates: list[ModelCandidate] = []
        for descriptor in selected:
            if descriptor.metadata.get("archived") is True:
                continue
            context_window_tokens = descriptor.context_window_tokens or args.context_window_tokens
            max_output_tokens = descriptor.max_output_tokens or min(
                args.model_max_output_tokens,
                context_window_tokens,
            )
            candidates.append(
                descriptor.to_candidate(
                    context_window_tokens=context_window_tokens,
                    max_output_tokens=min(max_output_tokens, context_window_tokens),
                    quality=args.quality,
                    latency_ms=args.latency_ms,
                    cost_per_million_tokens=args.cost_per_million_tokens,
                    reliability=args.reliability,
                    capabilities=capabilities,
                )
            )
        if not candidates:
            raise ValueError("provider discovery returned no selectable models")
        return tuple(candidates)
    if not models:
        raise ValueError("at least one --model is required unless --discover-models is enabled")
    candidates = tuple(
        ModelCandidate(
            provider=args.provider,
            model=model,
            context_window_tokens=args.context_window_tokens,
            max_output_tokens=min(args.model_max_output_tokens, args.context_window_tokens),
            quality=args.quality,
            latency_ms=args.latency_ms,
            cost_per_million_tokens=args.cost_per_million_tokens,
            reliability=args.reliability,
            capabilities=capabilities,
        )
        for model in models
    )
    return candidates


def _persisted_candidate_args(args: argparse.Namespace) -> tuple[ModelCandidate, ...]:
    if not args.use_inventory:
        raise ValueError("persisted model catalogue was not requested")
    if args.discover_models:
        raise ValueError("--use-inventory and --discover-models cannot be combined")
    if args.inventory_store is None:
        raise ValueError("--inventory-store is required with --use-inventory")
    catalogue = AutonomousModelInventoryStore(args.inventory_store).load_catalogue()
    if catalogue is None:
        raise ValueError("inventory store has no rehydratable model catalogue")
    rows = catalogue.candidates(providers=(args.provider,), enabled_only=True)
    requested = tuple(args.model or ())
    if requested:
        available = {row["model"] for row in rows}
        missing = tuple(model for model in requested if model not in available)
        if missing:
            raise ValueError("requested model was not found in the persisted catalogue")
        rows = [row for row in rows if row["model"] in requested]
    if not rows:
        raise ValueError("persisted catalogue has no enabled models for the selected provider")
    return tuple(ModelCandidate.from_mapping(row) for row in rows)


def _discover_descriptors(
    runtime: LLMRuntime,
    args: argparse.Namespace,
    session: Any,
) -> tuple[ProviderModelDescriptor, ...]:
    if not args.approve_provider_call:
        raise ValueError("model discovery requires --approve-provider-call")
    credential = (
        None
        if not runtime.provider_requires_credential(args.provider)
        else session.handle(args.provider)
    )
    return runtime.discover_models(
        args.provider,
        credential=credential,
        path=args.models_path,
        limit=args.model_limit,
    )


def _inventory_prior_factory(
    args: argparse.Namespace,
) -> Callable[[ProviderModelDescriptor], Mapping[str, Any]]:
    capabilities = tuple(args.model_capability or ())

    def build(descriptor: ProviderModelDescriptor) -> Mapping[str, Any]:
        context_window_tokens = descriptor.context_window_tokens or args.context_window_tokens
        max_output_tokens = descriptor.max_output_tokens or min(
            args.model_max_output_tokens,
            context_window_tokens,
        )
        return {
            "context_window_tokens": context_window_tokens,
            "max_output_tokens": min(max_output_tokens, context_window_tokens),
            "quality": args.quality,
            "latency_ms": args.latency_ms,
            "cost_per_million_tokens": args.cost_per_million_tokens,
            "reliability": args.reliability,
            "capabilities": capabilities,
            "enabled": descriptor.metadata.get("archived") is not True,
        }

    return build


def _credential_reader(reader: Callable[[str], str] | None) -> Callable[[str], str]:
    return reader if reader is not None else getpass.getpass


def _collect_credentials(
    args: argparse.Namespace,
    session: Any,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
) -> None:
    if not session.onboarding.runtime.provider_requires_credential(args.provider):
        return
    if args.credential_source == "environment":
        session.configure_from_environment(
            args.provider,
            variable=args.credential_env,
            environ=environ,
        )
        return
    session.configure_from_prompt(
        args.provider,
        prompt=f"{args.provider} API key (input hidden): ",
        reader=_credential_reader(reader),
    )


def _parse_mcp_command(value: str) -> tuple[str, ...]:
    try:
        command = tuple(shlex.split(value, posix=True))
    except ValueError as error:
        raise ValueError("--mcp-command has invalid quoting") from error
    if not command:
        raise ValueError("--mcp-command must contain an executable")
    return command


def _catalogue() -> dict[str, Any]:
    agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime())
    return {
        "schema": CLI_SCHEMA,
        "command": "catalogue",
        "domains": agent.domains(),
        "workflows": agent.workflows(),
        "domain_packs": agent.domain_packs(),
        "evaluators": [profile.to_dict() for profile in builtin_autonomous_domain_evaluator_profiles()],
        "selection": {
            "model_selection": "caller_declared_candidates_then_runtime_gates_and_adaptive_priors",
            "provider_io": "disabled",
            "credential_posture": "no_credentials_collected",
        },
        "secret_material": "never_returned",
    }


def _evidence_plan(args: argparse.Namespace) -> dict[str, Any]:
    agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime())
    plan = agent.evidence_plan(
        _domains(args.domain),
        available_evidence=tuple(args.available_evidence or ()),
    )
    return plan.to_dict()


def _route(args: argparse.Namespace) -> dict[str, Any]:
    agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime())
    return {
        "schema": CLI_SCHEMA,
        "command": "route",
        "route": agent.route(task=args.task, hints=tuple(args.hint or ())).to_dict(),
        "authorization": "routing_evidence_only; no_tools_or_effects_authorized",
        "secret_material": "never_returned",
    }


def _provider_status(args: argparse.Namespace) -> dict[str, Any]:
    runtime, onboarding = _runtime_with_provider(args)
    return {
        "schema": CLI_SCHEMA,
        "command": "provider-status",
        "status": onboarding.status(args.provider),
        "instructions": onboarding.instructions(args.provider).to_dict(),
        "provider": runtime.provider_metadata()[0],
        "secret_material": "never_returned",
    }


def _discover_models(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
) -> dict[str, Any]:
    if not args.approve_provider_call:
        raise ValueError("model discovery requires --approve-provider-call")
    runtime, onboarding = _runtime_with_provider(args)
    with onboarding.start_session(ttl_seconds=args.ttl_seconds) as session:
        _collect_credentials(args, session, environ=environ, reader=reader)
        descriptors = _discover_descriptors(runtime, args, session)
        provider_status = runtime.provider_status(args.provider)
    return {
        "schema": CLI_SCHEMA,
        "command": "discover-models",
        "provider": args.provider,
        "models": [descriptor.to_dict() for descriptor in descriptors],
        "model_count": len(descriptors),
        "provider_status": provider_status,
        "credential_session": session.status().to_dict(),
        "authorization": {"model_discovery_approved": args.approve_provider_call},
        "secret_material": "never_returned",
    }


def _refresh_models(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
) -> dict[str, Any]:
    if not args.approve_provider_call:
        raise ValueError("model inventory refresh requires --approve-provider-call")
    runtime, onboarding = _runtime_with_provider(args)
    snapshot_store = (
        AutonomousModelInventoryStore(args.inventory_store)
        if args.inventory_store is not None
        else None
    )
    with onboarding.start_session(ttl_seconds=args.ttl_seconds) as session:
        _collect_credentials(args, session, environ=environ, reader=reader)
        agent = AutonomousAgent(_OfflineWorkspace(), runtime, model_catalogue=ModelCatalogue())
        snapshot = agent.refresh_model_inventory(
            credentials=session,
            providers=(args.provider,),
            prior_factory=_inventory_prior_factory(args),
            limit=args.model_limit,
            snapshot_store=snapshot_store,
            refresh_id=args.refresh_id,
            raise_on_error=args.raise_on_error,
        )
        provider_status = runtime.provider_status(args.provider)
    return {
        "schema": CLI_SCHEMA,
        "command": "refresh-models",
        "provider": args.provider,
        "snapshot": snapshot,
        "provider_status": provider_status,
        "inventory_store": {
            "persisted": snapshot_store is not None,
            "snapshot_digest": snapshot.get("snapshot_digest"),
        },
        "credential_session": session.status().to_dict(),
        "authorization": {"model_inventory_refresh_approved": args.approve_provider_call},
        "secret_material": "never_returned",
    }


def _inventory_status(args: argparse.Namespace) -> dict[str, Any]:
    store = AutonomousModelInventoryStore(args.inventory_store)
    snapshot = store.load()
    catalogue = None if snapshot is None else store.load_catalogue()
    return {
        "schema": CLI_SCHEMA,
        "command": "inventory-status",
        "snapshot": None if snapshot is None else snapshot.to_dict(),
        "catalogue": None if catalogue is None else catalogue.to_dict(),
        "available": snapshot is not None,
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "secret_material": "never_returned",
    }


def _state_status(args: argparse.Namespace) -> dict[str, Any]:
    if args.health_store is None and args.learning_store is None:
        raise ValueError("state-status requires --health-store or --learning-store")
    health: dict[str, Any] = {
        "configured": args.health_store is not None,
        "available": False,
        "provider_health": {},
        "model_health": {},
    }
    if args.health_store is not None and os.path.exists(args.health_store):
        health_ledger = ProviderHealthLedger(args.health_store)
        health.update(
            {
                "available": True,
                "provider_health": health_ledger.health_snapshot(),
                "model_health": health_ledger.model_health_snapshot(),
                "record_count": len(health_ledger.records()),
            }
        )
    learning: dict[str, Any] = {
        "configured": args.learning_store is not None,
        "available": False,
        "record_count": 0,
        "latest_state": None,
        "domain_learning": {},
    }
    if args.learning_store is not None and os.path.exists(args.learning_store):
        learning_ledger = SQLiteBrainLearningLedger(args.learning_store)
        try:
            learning_agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime(), ledger=learning_ledger)
            learning.update(
                {
                    "available": True,
                    "record_count": len(learning_ledger.records()),
                    "latest_state": learning_ledger.latest_state(),
                    "domain_learning": {
                        domain: learning_agent.domain_learning_state(domain)
                        for domain in AUTONOMOUS_DOMAINS
                    },
                }
            )
        finally:
            learning_ledger.close()
    return {
        "schema": CLI_SCHEMA,
        "command": "state-status",
        "health": health,
        "learning": learning,
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "retention": "value_only_health_and_bandit_metadata",
        "secret_material": "never_returned",
    }


def _learning_episode_projection(
    episode: BrainLearningEpisode,
    *,
    status: str | None = None,
) -> dict[str, Any]:
    """Project a pending episode without returning its evaluator input envelope."""

    evaluation_input = episode.evaluation_input
    selected_model = evaluation_input.get("selected_model")
    selected = (
        {
            "provider": selected_model.get("provider"),
            "model": selected_model.get("model"),
        }
        if isinstance(selected_model, Mapping)
        else None
    )
    return {
        "schema": "aurora-learning-episode-status/0.1",
        "episode_id": episode.episode_id,
        "status": episode.status if status is None else status,
        "run_id": episode.run_id,
        "result_kind": episode.result_kind,
        "arm_id": episode.arm_id,
        "selected_model": selected,
        "context_digest": evaluation_input.get("context_digest"),
        "selection_digest": evaluation_input.get("selection_digest"),
        "prompt_digest": evaluation_input.get("prompt_digest"),
        "plan_digest": evaluation_input.get("plan_digest"),
        "outcome_digest": evaluation_input.get(
            "learning_outcome_digest", evaluation_input.get("outcome_digest")
        ),
        "evidence_digest": episode.evidence_digest,
        "retention": "identity_and_digests_only; evaluator_input_not_returned",
    }


def _learning_replay_projection(replay: Mapping[str, Any]) -> dict[str, Any]:
    """Return only the stable metadata needed to audit a settlement replay."""

    allowed = (
        "schema",
        "episode_id",
        "trajectory_id",
        "trajectory_step",
        "trajectory_length",
        "result_kind",
        "run_id",
        "outcome_digest",
        "evaluation_input_digest",
        "evidence_digest",
        "evaluator_id",
        "evaluator_version",
        "decision_digest",
        "raw_reward",
        "credited_reward",
        "discount",
        "terminal_reward",
        "retention",
    )
    return {key: replay[key] for key in allowed if key in replay}


def _learning_report_projection(report: Mapping[str, Any]) -> dict[str, Any]:
    """Keep the CLI settlement response value-only even if a workspace adds fields later."""

    projected: dict[str, Any] = {
        "status": report.get("status"),
        "next_state": report.get("next_state"),
        "learning_evidence": report.get("learning_evidence"),
    }
    if "idempotent" in report:
        projected["idempotent"] = report.get("idempotent")
    projected["retention"] = "value_only_learning_report; provider_payload_not_returned"
    return projected


def _learning_status(args: argparse.Namespace) -> dict[str, Any]:
    """Inspect pending and settled learning metadata without opening a provider session."""

    if not os.path.exists(args.learning_store):
        return {
            "schema": CLI_SCHEMA,
            "command": "learning-status",
            "available": False,
            "learning_store": args.learning_store,
            "pending_episodes": [],
            "replays": [],
            "authorization": "metadata_read_only; no_provider_or_credential_access",
            "retention": "value_only_learning_metadata",
            "secret_material": "never_returned",
        }
    ledger = SQLiteBrainLearningLedger(args.learning_store)
    try:
        pending = ledger.pending_episodes(limit=args.limit)
        replays = ledger.replays(limit=args.limit)
        all_replays = ledger.replays(limit=ledger.max_records)
        settled_ids = {
            replay.get("episode_id")
            for replay in all_replays
            if isinstance(replay.get("episode_id"), str)
        }
        selected = None
        if args.episode_id is not None:
            episode = ledger.episode(args.episode_id)
            selected = None if episode is None else _learning_episode_projection(
                episode,
                status="settled" if episode.episode_id in settled_ids else "pending",
            )
        agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime(), ledger=ledger)
        return {
            "schema": CLI_SCHEMA,
            "command": "learning-status",
            "available": True,
            "learning_store": args.learning_store,
            "record_count": len(ledger.records()),
            "pending_episode_count": len(pending),
            "pending_episodes": [_learning_episode_projection(episode) for episode in pending],
            "settled_episode_count": len(settled_ids),
            "selected_episode": selected,
            "replays": [_learning_replay_projection(replay) for replay in replays],
            "latest_state": ledger.latest_state(),
            "domain_learning": {
                domain: agent.domain_learning_state(domain)
                for domain in AUTONOMOUS_DOMAINS
            },
            "authorization": "metadata_read_only; no_provider_or_credential_access",
            "retention": "value_only_learning_metadata",
            "secret_material": "never_returned",
        }
    finally:
        ledger.close()


def _execution_event_projection(row: Mapping[str, Any]) -> dict[str, Any]:
    """Project one hash-verified execution transition without transient execution values."""

    event = row.get("event")
    if not isinstance(event, Mapping):
        return {
            "sequence": row.get("sequence"),
            "event_digest": row.get("event_digest"),
            "retention": "metadata_only_hash_chained",
        }
    allowed = (
        "execution_id",
        "kind",
        "domain",
        "capability",
        "risk_class",
        "status",
        "policy_digest",
    )
    return {
        "sequence": row.get("sequence"),
        "event_digest": row.get("event_digest"),
        **{key: event[key] for key in allowed if key in event},
        "retention": "metadata_only_hash_chained",
    }


def _execution_status(args: argparse.Namespace) -> dict[str, Any]:
    """Inspect hash-verified execution checkpoints without contacting a provider."""

    if not os.path.exists(args.execution_store):
        return {
            "schema": CLI_SCHEMA,
            "command": "execution-status",
            "available": False,
            "execution_store": args.execution_store,
            "executions": [],
            "events": [],
            "authorization": "metadata_read_only; no_provider_or_credential_access",
            "retention": "metadata_only_hash_chained_execution_state",
            "secret_material": "never_returned",
        }
    journal = AutonomousExecutionJournal(args.execution_store)
    rows = journal.events(execution_id=args.execution_id, limit=args.limit)
    execution_ids = sorted({
        row.get("event", {}).get("execution_id")
        for row in rows
        if isinstance(row.get("event"), Mapping)
        and isinstance(row.get("event", {}).get("execution_id"), str)
    })
    executions: list[dict[str, Any]] = []
    for execution_id in execution_ids:
        state = journal.state(execution_id)
        if state is None:
            continue
        executions.append({
            "execution_id": execution_id,
            "state": state.to_dict(),
        })
    return {
        "schema": CLI_SCHEMA,
        "command": "execution-status",
        "available": True,
        "execution_store": args.execution_store,
        "execution_id_filter": args.execution_id,
        "event_count": len(rows),
        "executions": executions,
        "events": [_execution_event_projection(row) for row in rows],
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "retention": "metadata_only_hash_chained_execution_state",
        "secret_material": "never_returned",
    }


def _workflow_checkpoint_projection(
    checkpoint: AutonomousWorkflowCheckpoint,
) -> dict[str, Any]:
    """Project a workflow checkpoint without returning structured stage values."""

    stages = [
        {
            "stage_id": stage["stage_id"],
            "status": stage["status"],
            "execution_status": stage["execution_status"],
            "attempt": stage["attempt"],
            "response_digest": stage["response_digest"],
            "stage_execution_plan_digest": stage.get("stage_execution_plan_digest"),
            "evidence_count": len(stage.get("evidence", ())),
            "uncertainty_count": len(stage.get("uncertainty", ())),
            "selected_tool_count": len(stage.get("stage_selected_tool_names", ())),
            "capability_contract_count": len(stage.get("stage_capability_contract_digests", ())),
        }
        for stage in checkpoint.stages
    ]
    completed = set(checkpoint.completed_stage_ids)
    return {
        "schema": checkpoint.to_dict()["schema"],
        "run_id": checkpoint.run_id,
        "task_digest": checkpoint.task_digest,
        "workflow_id": checkpoint.workflow_id,
        "workflow_digest": checkpoint.workflow_digest,
        "plan_refinement_digest": checkpoint.plan_refinement_digest,
        "stage_count": len(stages),
        "completed_stage_ids": list(checkpoint.completed_stage_ids),
        "remaining_stage_ids": [stage["stage_id"] for stage in stages if stage["stage_id"] not in completed],
        "stages": stages,
        "checkpoint_digest": checkpoint.checkpoint_digest,
        "retention": "stage_status_counts_and_digests_only; structured_values_not_returned",
    }


def _workflow_checkpoint_store_payload(
    checkpoint: AutonomousWorkflowCheckpoint,
) -> dict[str, Any]:
    checkpoint_payload = checkpoint.to_dict()
    payload: dict[str, Any] = {
        "schema": WORKFLOW_CHECKPOINT_STORE_SCHEMA,
        "checkpoint": checkpoint_payload,
        "checkpoint_digest": checkpoint.checkpoint_digest,
        "retention": "caller_owned_structured_stage_metadata; no_task_or_provider_transcript",
    }
    payload["store_digest"] = content_digest(payload)
    return payload


def _load_workflow_checkpoint(path_value: str) -> AutonomousWorkflowCheckpoint:
    """Load and verify one caller-owned workflow checkpoint before any provider access."""

    path = Path(path_value)
    if not path.exists():
        raise ValueError("workflow resume requires an existing checkpoint store")
    if not path.is_file():
        raise ValueError("workflow checkpoint store must be a regular file")
    if path.stat().st_size > _MAX_WORKFLOW_CHECKPOINT_STORE_BYTES:
        raise ValueError("workflow checkpoint store exceeds its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("workflow checkpoint store is unreadable") from error
    if not isinstance(raw, Mapping) or raw.get("schema") != WORKFLOW_CHECKPOINT_STORE_SCHEMA:
        raise ValueError("workflow checkpoint store has an invalid schema")
    supplied_store_digest = raw.get("store_digest")
    unsigned = dict(raw)
    unsigned.pop("store_digest", None)
    if supplied_store_digest != content_digest(unsigned):
        raise ValueError("workflow checkpoint store digest does not match its contents")
    checkpoint_payload = raw.get("checkpoint")
    if not isinstance(checkpoint_payload, Mapping):
        raise ValueError("workflow checkpoint store is missing its checkpoint")
    checkpoint = AutonomousWorkflowCheckpoint.from_dict(checkpoint_payload)
    if raw.get("checkpoint_digest") != checkpoint.checkpoint_digest:
        raise ValueError("workflow checkpoint store checkpoint digest does not match")
    return checkpoint


def _persist_workflow_checkpoint(
    path_value: str,
    checkpoint: AutonomousWorkflowCheckpoint,
) -> None:
    """Atomically replace a caller-owned checkpoint store after validating the checkpoint."""

    payload = _workflow_checkpoint_store_payload(
        AutonomousWorkflowCheckpoint.from_dict(checkpoint.to_dict())
    )
    encoded = json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        indent=2,
        allow_nan=False,
    ) + "\n"
    if len(encoded.encode("utf-8")) > _MAX_WORKFLOW_CHECKPOINT_STORE_BYTES:
        raise ValueError("workflow checkpoint store exceeds its bounded size")
    destination = Path(path_value)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=str(destination.parent),
            prefix=f".{destination.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_name = temporary.name
            temporary.write(encoded)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, destination)
        temporary_name = None
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def _workflow_checkpoint_from_result(result: Any) -> AutonomousWorkflowCheckpoint | None:
    """Find the typed checkpoint nested in automatic workflow and learning results."""

    pending: list[Any] = [result]
    visited: set[int] = set()
    while pending and len(visited) < 16:
        current = pending.pop(0)
        identity = id(current)
        if identity in visited:
            continue
        visited.add(identity)
        if isinstance(current, AutonomousWorkflowCheckpoint):
            return current
        checkpoint = getattr(current, "checkpoint", None)
        if isinstance(checkpoint, AutonomousWorkflowCheckpoint):
            return checkpoint
        for attribute in ("result", "workflow"):
            nested = getattr(current, attribute, None)
            if nested is not None and not isinstance(nested, (str, bytes, Mapping)):
                pending.append(nested)
    return None


def _workflow_status(args: argparse.Namespace) -> dict[str, Any]:
    """Inspect a digest-verified workflow checkpoint without opening a provider session."""

    if not os.path.exists(args.workflow_checkpoint_store):
        return {
            "schema": CLI_SCHEMA,
            "command": "workflow-status",
            "available": False,
            "workflow_checkpoint_store": args.workflow_checkpoint_store,
            "checkpoint": None,
            "authorization": "metadata_read_only; no_provider_or_credential_access",
            "retention": "stage_status_counts_and_digests_only",
            "secret_material": "never_returned",
        }
    checkpoint = _load_workflow_checkpoint(args.workflow_checkpoint_store)
    return {
        "schema": CLI_SCHEMA,
        "command": "workflow-status",
        "available": True,
        "workflow_checkpoint_store": args.workflow_checkpoint_store,
        "checkpoint": _workflow_checkpoint_projection(checkpoint),
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "retention": "stage_status_counts_and_digests_only",
        "secret_material": "never_returned",
    }


class _BatchCheckpointFileStore:
    """Atomic, digest-bound file adapter for the metadata-only batch checkpoint."""

    def __init__(self, path_value: str) -> None:
        self.path = Path(path_value)

    def read(self) -> dict[str, Any] | None:
        if not self.path.exists():
            return None
        if not self.path.is_file() or self.path.stat().st_size > _MAX_BATCH_RESULT_MANIFEST_BYTES:
            raise ValueError("batch checkpoint store is outside its bounded file contract")
        try:
            raw = json.loads(self.path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise ValueError("batch checkpoint store is unreadable") from error
        if not isinstance(raw, Mapping) or raw.get("schema") != BATCH_CHECKPOINT_STORE_SCHEMA:
            raise ValueError("batch checkpoint store has an invalid schema")
        supplied_digest = raw.get("store_digest")
        unsigned = dict(raw)
        unsigned.pop("store_digest", None)
        if supplied_digest != content_digest(unsigned):
            raise ValueError("batch checkpoint store digest does not match its contents")
        checkpoint_payload = raw.get("checkpoint")
        if not isinstance(checkpoint_payload, Mapping):
            raise ValueError("batch checkpoint store is missing its checkpoint")
        checkpoint = AutonomousBatchCheckpoint.from_dict(checkpoint_payload)
        if raw.get("checkpoint_digest") != checkpoint.checkpoint_digest:
            raise ValueError("batch checkpoint store checkpoint digest does not match")
        return checkpoint.to_dict()

    def write(self, checkpoint: AutonomousBatchCheckpoint | Mapping[str, Any]) -> None:
        verified = (
            AutonomousBatchCheckpoint.from_dict(checkpoint.to_dict())
            if isinstance(checkpoint, AutonomousBatchCheckpoint)
            else AutonomousBatchCheckpoint.from_dict(checkpoint)
        )
        unsigned: dict[str, Any] = {
            "schema": BATCH_CHECKPOINT_STORE_SCHEMA,
            "checkpoint": verified.to_dict(),
            "checkpoint_digest": verified.checkpoint_digest,
            "retention": "request_and_result_digests_only; task_text_and_provider_values_not_persisted",
        }
        payload = {**unsigned, "store_digest": content_digest(unsigned)}
        encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
        if len(encoded.encode("utf-8")) > _MAX_BATCH_RESULT_MANIFEST_BYTES:
            raise ValueError("batch checkpoint store exceeds its bounded size")
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary_name: str | None = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=str(self.path.parent),
                prefix=f".{self.path.name}.",
                suffix=".tmp",
                delete=False,
            ) as temporary:
                temporary_name = temporary.name
                temporary.write(encoded)
                temporary.flush()
                os.fsync(temporary.fileno())
            os.replace(temporary_name, self.path)
            temporary_name = None
        finally:
            if temporary_name is not None:
                try:
                    os.unlink(temporary_name)
                except FileNotFoundError:
                    pass


def _batch_checkpoint_projection(checkpoint: AutonomousBatchCheckpoint) -> dict[str, Any]:
    """Project batch progress without returning tasks, options, or result payloads."""

    return {
        "schema": checkpoint.to_dict()["schema"],
        "job_id": checkpoint.job_id,
        "mode": checkpoint.mode,
        "batch_input_digest": checkpoint.batch_input_digest,
        "request_count": len(checkpoint.request_digests),
        "completed_indices": list(checkpoint.completed_indices),
        "completed_count": len(checkpoint.completed_indices),
        "remaining_count": len(checkpoint.request_digests) - len(checkpoint.completed_indices),
        "completed_result_digests": list(checkpoint.completed_result_digests),
        "max_parallelism": checkpoint.max_parallelism,
        "stop_on_error": checkpoint.stop_on_error,
        "status": checkpoint.status,
        "checkpoint_digest": checkpoint.checkpoint_digest,
        "retention": "request_and_result_digests_only; tasks_and_provider_values_not_returned",
    }


def _batch_status(args: argparse.Namespace) -> dict[str, Any]:
    """Inspect a batch checkpoint without opening credentials, MCP, or a provider."""

    if not os.path.exists(args.batch_checkpoint_store):
        return {
            "schema": CLI_SCHEMA,
            "command": "batch-status",
            "available": False,
            "batch_checkpoint_store": args.batch_checkpoint_store,
            "checkpoint": None,
            "authorization": "metadata_read_only; no_provider_or_credential_access",
            "retention": "request_and_result_digests_only",
            "secret_material": "never_returned",
        }
    store = _BatchCheckpointFileStore(args.batch_checkpoint_store)
    raw = store.read()
    if raw is None:
        raise ValueError("batch checkpoint store unexpectedly returned no checkpoint")
    checkpoint = AutonomousBatchCheckpoint.from_dict(raw)
    return {
        "schema": CLI_SCHEMA,
        "command": "batch-status",
        "available": True,
        "batch_checkpoint_store": args.batch_checkpoint_store,
        "checkpoint": _batch_checkpoint_projection(checkpoint),
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "retention": "request_and_result_digests_only",
        "secret_material": "never_returned",
    }


class _CliRehydratedBatchResult:
    """Metadata-only successful result used to rehydrate independent batch items."""

    def __init__(self, status: str) -> None:
        self.status = status


def _batch_manifest_payload(
    checkpoint: AutonomousBatchCheckpoint,
    result: AutonomousBatchResult,
) -> dict[str, Any]:
    expected_digests = dict(zip(checkpoint.completed_indices, checkpoint.completed_result_digests))
    items = []
    for item in result.items:
        if item.status != "succeeded":
            continue
        result_status = item.result_status
        if (
            not isinstance(result_status, str)
            or not result_status
            or len(result_status) > 128
            or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in result_status)
        ):
            raise ValueError("successful batch result is missing a bounded result status")
        item_projection = item.to_dict()
        item_digest = content_digest(item_projection)
        if item.index not in expected_digests or expected_digests[item.index] != item_digest:
            raise ValueError("batch result does not match the persisted checkpoint")
        items.append(
            {
                "index": item.index,
                "result_status": result_status,
                "result_digest": item_digest,
            }
        )
    if set(item["index"] for item in items) != set(checkpoint.completed_indices):
        raise ValueError("batch result does not cover every checkpointed successful item")
    unsigned: dict[str, Any] = {
        "schema": BATCH_RESULT_MANIFEST_SCHEMA,
        "job_id": checkpoint.job_id,
        "checkpoint_digest": checkpoint.checkpoint_digest,
        "batch_input_digest": checkpoint.batch_input_digest,
        "items": items,
        "retention": "successful_result_status_and_item_digests_only; result_values_not_persisted",
    }
    return {**unsigned, "manifest_digest": content_digest(unsigned)}


def _write_batch_result_manifest(
    path_value: str,
    checkpoint: AutonomousBatchCheckpoint,
    result: AutonomousBatchResult,
) -> None:
    payload = _batch_manifest_payload(checkpoint, result)
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
    if len(encoded.encode("utf-8")) > _MAX_BATCH_RESULT_MANIFEST_BYTES:
        raise ValueError("batch result manifest exceeds its bounded size")
    path = Path(path_value)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=str(path.parent),
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temporary:
            temporary_name = temporary.name
            temporary.write(encoded)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
        temporary_name = None
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def _load_batch_rehydrator(
    path_value: str,
    checkpoint: AutonomousBatchCheckpoint,
) -> Callable[[Any], Any]:
    path = Path(path_value)
    if not path.exists() or not path.is_file() or path.stat().st_size > _MAX_BATCH_RESULT_MANIFEST_BYTES:
        raise ValueError("batch resume requires an existing bounded result manifest")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("batch result manifest is unreadable") from error
    if not isinstance(raw, Mapping) or raw.get("schema") != BATCH_RESULT_MANIFEST_SCHEMA:
        raise ValueError("batch result manifest has an invalid schema")
    unsigned = dict(raw)
    supplied_digest = unsigned.pop("manifest_digest", None)
    if supplied_digest != content_digest(unsigned):
        raise ValueError("batch result manifest digest does not match its contents")
    if raw.get("job_id") != checkpoint.job_id or raw.get("checkpoint_digest") != checkpoint.checkpoint_digest or raw.get("batch_input_digest") != checkpoint.batch_input_digest:
        raise ValueError("batch result manifest does not match the checkpoint")
    raw_items = raw.get("items")
    if not isinstance(raw_items, Sequence) or isinstance(raw_items, (str, bytes)):
        raise ValueError("batch result manifest items are malformed")
    entries: dict[int, Mapping[str, Any]] = {}
    for raw_item in raw_items:
        if not isinstance(raw_item, Mapping):
            raise ValueError("batch result manifest item is malformed")
        index = raw_item.get("index")
        result_status = raw_item.get("result_status")
        result_digest = raw_item.get("result_digest")
        if (
            not isinstance(index, int)
            or isinstance(index, bool)
            or not isinstance(result_status, str)
            or not result_status
            or len(result_status) > 128
            or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in result_status)
            or not isinstance(result_digest, str)
            or len(result_digest) != 64
            or any(character not in "0123456789abcdef" for character in result_digest)
        ):
            raise ValueError("batch result manifest item is outside its contract")
        if index in entries or index not in checkpoint.completed_indices:
            raise ValueError("batch result manifest item index is not checkpointed")
        entries[index] = raw_item
    if set(entries) != set(checkpoint.completed_indices):
        raise ValueError("batch result manifest does not cover every completed item")

    def rehydrate(context: Any) -> Any:
        if context.job_id != checkpoint.job_id or context.mode != checkpoint.mode:
            raise ValueError("batch result manifest context does not match the checkpoint")
        entry = entries.get(context.index)
        if entry is None:
            raise ValueError("batch result manifest is missing the requested item")
        result = _CliRehydratedBatchResult(entry["result_status"])
        item = AutonomousBatchItem(
            index=context.index,
            status="succeeded",
            task_digest=context.task_digest,
            result=result,
        )
        item_digest = content_digest(item.to_dict())
        if item_digest != entry["result_digest"] or item_digest != context.expected_result_digest:
            raise ValueError("batch result manifest item digest does not match the checkpoint")
        return result

    return rehydrate


def _batch_request_json_safe(value: Any, *, depth: int = 0) -> None:
    """Reject credential-shaped request-file fields before they reach the batch engine."""

    if depth > 32:
        raise ValueError("batch request file is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ValueError("batch request file contains an invalid field")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in {
                "apikey",
                "authorization",
                "bearer",
                "credential",
                "credentials",
                "password",
                "privatekey",
                "secret",
                "secretkey",
                "token",
                "accesstoken",
                "refreshtoken",
            } or normalized.startswith("gsk") or normalized.startswith("skproj"):
                raise ValueError("batch request file contains credential-shaped fields")
            _batch_request_json_safe(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _batch_request_json_safe(child, depth=depth + 1)
    elif isinstance(value, (str, bool, int, float)) or value is None:
        if isinstance(value, float) and (value != value or value in {float("inf"), float("-inf")}):
            raise ValueError("batch request file contains a non-finite number")
    else:
        raise ValueError("batch request file contains a non-JSON value")


def _load_batch_requests(args: argparse.Namespace) -> tuple[str, str, list[dict[str, Any]]]:
    path = Path(args.requests_file)
    if not path.exists() or not path.is_file() or path.stat().st_size > _MAX_BATCH_REQUEST_FILE_BYTES:
        raise ValueError("batch requests file is missing or outside its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("batch requests file is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_BATCH_REQUESTS_SCHEMA:
        raise ValueError("batch requests file has an invalid schema")
    mode = raw.get("mode")
    if args.batch_mode is not None and mode != args.batch_mode:
        raise ValueError("--batch-mode does not match the request file")
    if mode not in {"domain", "auto", "cross_domain"}:
        raise ValueError("batch requests mode must be domain, auto, or cross_domain")
    job_id = raw.get("job_id", args.job_id)
    if job_id != args.job_id:
        raise ValueError("--job-id does not match the request file")
    if not isinstance(job_id, str) or not job_id.strip() or len(job_id.encode("utf-8")) > 256:
        raise ValueError("batch job_id is outside its bounded contract")
    requests = raw.get("requests")
    if not isinstance(requests, list) or not 1 <= len(requests) <= 64:
        raise ValueError("batch requests must contain between 1 and 64 entries")
    allowed_fields = {"task", "domain", "subtasks", "options", "model_candidates", "execution_id"}
    normalized_requests: list[dict[str, Any]] = []
    for index, request in enumerate(requests):
        if not isinstance(request, Mapping):
            raise ValueError(f"batch request {index} must be an object")
        unknown = sorted(set(request).difference(allowed_fields))
        if unknown:
            raise ValueError(f"batch request {index} contains unsupported fields")
        task = request.get("task")
        if not isinstance(task, str) or not task.strip() or len(task.encode("utf-8")) > 16_000:
            raise ValueError(f"batch request {index} task is outside its bounded contract")
        if mode == "domain":
            domain = request.get("domain")
            if domain not in AUTONOMOUS_DOMAINS:
                raise ValueError(f"batch request {index} has an unsupported domain")
            if "subtasks" in request:
                raise ValueError(f"batch request {index} cannot contain subtasks in domain mode")
        elif mode == "cross_domain":
            subtasks = request.get("subtasks")
            if not isinstance(subtasks, list) or not subtasks:
                raise ValueError(f"cross-domain batch request {index} requires subtasks")
            if "domain" in request:
                raise ValueError(f"cross-domain batch request {index} cannot contain domain")
        else:
            if "domain" in request or "subtasks" in request:
                raise ValueError(f"automatic batch request {index} cannot preselect a domain or subtasks")
        options = request.get("options", {})
        if options is None:
            options = {}
        if not isinstance(options, Mapping):
            raise ValueError(f"batch request {index} options must be an object")
        if "credentials" in options:
            raise ValueError("batch request options cannot carry credentials")
        normalized = dict(request)
        normalized["options"] = dict(options)
        normalized_requests.append(normalized)
    return mode, job_id, normalized_requests


def _batch_run(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
    client_factory: Callable[..., Client] = Client,
) -> dict[str, Any]:
    mode, job_id, requests = _load_batch_requests(args)
    if args.resume_batch and args.batch_checkpoint_store is None:
        raise ValueError("--resume-batch requires --batch-checkpoint-store")
    if args.batch_result_manifest is not None and args.batch_checkpoint_store is None:
        raise ValueError("--batch-result-manifest requires --batch-checkpoint-store")
    if args.batch_checkpoint_store is not None:
        checkpoint_store: Any = _BatchCheckpointFileStore(args.batch_checkpoint_store)
        existing_checkpoint = checkpoint_store.read()
        if existing_checkpoint is not None and not args.resume_batch:
            raise ValueError("existing batch checkpoint requires --resume-batch")
        if args.resume_batch and existing_checkpoint is None:
            raise ValueError("--resume-batch requires an existing batch checkpoint")
    else:
        checkpoint_store = InMemoryAutonomousBatchCheckpointStore()
        existing_checkpoint = None
    command = _parse_mcp_command(args.mcp_command)
    if args.discover_models and not args.approve_provider_call:
        raise ValueError("model discovery requires --approve-provider-call")
    if args.use_inventory and args.discover_models:
        raise ValueError("--use-inventory and --discover-models cannot be combined")
    if args.use_inventory and args.inventory_store is None:
        raise ValueError("--inventory-store is required with --use-inventory")
    if args.batch_mode is not None and args.batch_mode not in {"domain", "auto", "cross_domain"}:
        raise ValueError("--batch-mode is unsupported")
    if args.learning_mode == "trajectory" and mode == "domain":
        raise ValueError("trajectory learning requires automatic or cross-domain batch mode")
    if args.workflow_execution and mode != "auto":
        raise ValueError("--workflow-execution requires automatic batch mode")
    if args.workflow_execution and not args.single_domain:
        raise ValueError("batch workflow execution requires --single-domain")
    if (args.workflow_max_stage_calls is not None or args.workflow_retry_blocked) and not args.workflow_execution:
        raise ValueError("batch workflow controls require --workflow-execution")
    if args.learning_mode is not None and mode == "cross_domain":
        raise ValueError("cross-domain batch learning must be declared per request in options")
    persisted_candidates = _persisted_candidate_args(args) if args.use_inventory else ()
    runtime, onboarding = _runtime_with_provider(args)
    session = onboarding.start_session(ttl_seconds=args.ttl_seconds)
    health_ledger = None
    learning_ledger = None
    execution_journal = None
    try:
        if args.health_store is not None:
            health_ledger = ProviderHealthLedger(args.health_store)
        if args.learning_store is not None:
            learning_ledger = SQLiteBrainLearningLedger(args.learning_store)
        if args.execution_store is not None:
            execution_journal = AutonomousExecutionJournal(args.execution_store)
        _collect_credentials(args, session, environ=environ, reader=reader)
        descriptors = _discover_descriptors(runtime, args, session) if args.discover_models else ()
        candidates = (
            _candidate_args(args, descriptors)
            if args.discover_models
            else persisted_candidates
            if args.use_inventory
            else _candidate_args(args)
        )
        catalogue = ModelCatalogue(candidates)
        client = client_factory(command, cwd=args.mcp_cwd, timeout=args.mcp_timeout)
        with client:
            agent_kwargs: dict[str, Any] = {"model_catalogue": catalogue}
            if learning_ledger is not None:
                agent_kwargs["ledger"] = learning_ledger
            if health_ledger is not None:
                agent_kwargs["health_ledger"] = health_ledger
            if execution_journal is not None:
                agent_kwargs["execution_journal"] = execution_journal
            agent = AutonomousAgent(_McpWorkspace(client), runtime, **agent_kwargs)
            needs_provider_tools = args.execution_mode in {"tool_loop", "mission"} or any(
                isinstance(request.get("options"), Mapping)
                and request["options"].get("execution_mode") in {"tool_loop", "mission"}
                for request in requests
            )
            provider_tools = _mcp_provider_tools(client) if needs_provider_tools else ()
            tool_authorizer = (
                _cli_tool_authorizer(
                    client,
                    approve_mission_dispatch=args.approve_mission_dispatch,
                )
                if provider_tools
                else None
            )
            controller = AutonomousBrainBatchJobController(agent, checkpoint_store)
            controller.restore()
            rehydrate_result = None
            if args.resume_batch and existing_checkpoint is not None:
                checkpoint = AutonomousBatchCheckpoint.from_dict(existing_checkpoint)
                if checkpoint.completed_indices:
                    manifest_path = args.batch_result_manifest or f"{args.batch_checkpoint_store}.results.json"
                    rehydrate_result = _load_batch_rehydrator(manifest_path, checkpoint)

            def options_factory(raw: Mapping[str, Any], _index: int) -> Mapping[str, Any]:
                options = dict(raw.get("options", {}))
                # Approval is an operator-level gate and cannot be smuggled in through the file.
                options["approve_provider_call"] = args.approve_provider_call
                options["approve_mission_dispatch"] = args.approve_mission_dispatch
                if args.execution_mode is not None:
                    options["execution_mode"] = args.execution_mode
                if args.max_steps is not None:
                    options["max_steps"] = args.max_steps
                if args.requested_output_tokens is not None:
                    options["requested_output_tokens"] = args.requested_output_tokens
                    options["max_output_tokens"] = args.requested_output_tokens
                if args.learning_mode is not None:
                    if mode == "auto":
                        options["learning_mode"] = args.learning_mode
                    elif args.learning_mode == "online":
                        options["learn"] = True
                if args.workflow_execution:
                    options["workflow_execution"] = True
                    options["allow_cross_domain"] = False
                    if args.workflow_max_stage_calls is not None:
                        options["workflow_max_stage_calls"] = args.workflow_max_stage_calls
                    if args.workflow_retry_blocked:
                        options["workflow_retry_blocked"] = True
                if provider_tools and options.get("execution_mode", "provider") in {"tool_loop", "mission"}:
                    options["provider_tools"] = provider_tools
                    options["tool_loop_options"] = {
                        "authorize_and_execute": tool_authorizer,
                    }
                return options

            run_payload = controller.run(
                requests,
                job_id=job_id,
                mode=mode,
                credentials=session,
                model_candidates=candidates,
                options_factory=options_factory,
                max_parallelism=args.max_parallelism,
                stop_on_error=args.stop_on_error,
                rehydrate_result=rehydrate_result,
            )
        batch = run_payload.get("batch") if isinstance(run_payload, Mapping) else None
        if not isinstance(batch, AutonomousBatchResult):
            raise ValueError("batch engine returned an invalid result")
        checkpoint_raw = checkpoint_store.read()
        checkpoint = None if checkpoint_raw is None else AutonomousBatchCheckpoint.from_dict(checkpoint_raw)
        manifest_path = None
        if args.batch_checkpoint_store is not None and checkpoint is not None:
            manifest_path = args.batch_result_manifest or f"{args.batch_checkpoint_store}.results.json"
            _write_batch_result_manifest(manifest_path, checkpoint, batch)
        return {
            "schema": CLI_SCHEMA,
            "command": "batch-run",
            "mode": mode,
            "job_id": job_id,
            "model_inventory": {
                "mode": "provider_discovery" if args.discover_models else "persisted_catalogue" if args.use_inventory else "caller_declared",
                "models": [descriptor.to_dict() for descriptor in descriptors],
                "model_count": len(descriptors),
                "candidates": [candidate.to_dict() for candidate in candidates] if args.use_inventory else [],
            },
            "batch": batch,
            "controller": run_payload.get("controller") if isinstance(run_payload, Mapping) else None,
            "batch_persistence": {
                "checkpoint_store": args.batch_checkpoint_store,
                "checkpoint_available": checkpoint is not None,
                "checkpoint_digest": None if checkpoint is None else checkpoint.checkpoint_digest,
                "resume_requested": args.resume_batch,
                "result_manifest": manifest_path,
                "retention": "checkpoint_digests_and_success_statuses_only; task_text_and_provider_values_not_persisted",
            },
            "provider_status": runtime.provider_status(args.provider),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": args.approve_provider_call,
                "mission_dispatch_approved": args.approve_mission_dispatch,
            },
            "secret_material": "never_returned",
        }
    finally:
        session.close()
        if learning_ledger is not None:
            learning_ledger.close()


def _settle_learning(
    args: argparse.Namespace,
    *,
    client_factory: Callable[..., Client],
) -> dict[str, Any]:
    """Apply a prevalidated evaluator projection through the caller-owned brain kernel."""

    if not os.path.exists(args.learning_store):
        raise ValueError("learning settlement requires an existing --learning-store")
    ledger = SQLiteBrainLearningLedger(args.learning_store)
    try:
        episode = ledger.episode(args.episode_id)
        if episode is None:
            raise ValueError("learning settlement episode was not found")
        decision = BrainEvaluatorDecision(
            evaluator_id=args.evaluator_id,
            evaluator_version=args.evaluator_version,
            reward=args.reward,
            passed=args.outcome == "passed",
            failed=args.outcome == "failed",
            feedback_digest=args.feedback_digest,
            failure_class=args.failure_class,
            evidence_digest=args.evidence_digest,
        )
        evaluator = BrainOutcomeEvaluator(
            lambda _value: decision.to_dict(),
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
        )
        command = _parse_mcp_command(args.mcp_command)
        runtime = LLMRuntime()
        client = client_factory(
            command,
            cwd=args.mcp_cwd,
            timeout=args.mcp_timeout,
        )
        with client:
            agent = AutonomousAgent(_McpWorkspace(client), runtime, ledger=ledger)
            settled_decision, report = agent.settle_learning_decision(
                episode,
                decision=decision,
                evaluator=evaluator,
            )
        return {
            "schema": CLI_SCHEMA,
            "command": "settle-learning",
            "episode": _learning_episode_projection(episode),
            "decision": settled_decision.to_dict(),
            "report": _learning_report_projection(report),
            "pending_episode_count_after": len(ledger.pending_episodes(limit=ledger.max_records)),
            "authorization": {
                "evaluator_projection": "caller_or_evaluator_worker_supplied_value_only_decision",
                "brain_kernel": "caller_workspace_brain_outcome_record_only",
                "provider_call": False,
                "credential_access": False,
            },
            "retention": "episode_identity_evaluator_decision_and_bandit_digests_only",
            "secret_material": "never_returned",
        }
    finally:
        ledger.close()


def _onboard(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
) -> dict[str, Any]:
    _runtime, onboarding = _runtime_with_provider(args)
    with onboarding.start_session(ttl_seconds=args.ttl_seconds) as session:
        _collect_credentials(
            args,
            session,
            environ=environ,
            reader=reader,
        )
        provider_statuses = session.provider_statuses()
        provider_status = (
            provider_statuses[0]
            if provider_statuses
            else onboarding.status(args.provider)
        )
    status = session.status().to_dict()
    return {
        "schema": CLI_SCHEMA,
        "command": "onboard",
        "session": status,
        "provider": provider_status,
        "session_closed": True,
        "secret_material": "never_returned",
    }


def _run(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
    client_factory: Callable[..., Client] = Client,
) -> dict[str, Any]:
    if (args.automatic and args.domain is not None) or (
        not args.automatic and args.domain is None
    ):
        raise ValueError("choose exactly one of --automatic or --domain")
    command = _parse_mcp_command(args.mcp_command)
    if args.discover_models and not args.approve_provider_call:
        raise ValueError("model discovery requires --approve-provider-call")
    if args.use_inventory and args.discover_models:
        raise ValueError("--use-inventory and --discover-models cannot be combined")
    if args.use_inventory and args.inventory_store is None:
        raise ValueError("--inventory-store is required with --use-inventory")
    if not args.automatic and args.learning_mode == "trajectory":
        raise ValueError("trajectory learning requires --automatic or a workflow execution API")
    workflow_controls_requested = any(
        (
            args.workflow_execution,
            args.workflow_retry_blocked,
            args.workflow_max_stage_calls is not None,
            args.workflow_checkpoint_store is not None,
            args.resume_workflow,
        )
    )
    if workflow_controls_requested and not args.automatic:
        raise ValueError("workflow controls require --automatic")
    if args.workflow_execution and not args.single_domain:
        raise ValueError("workflow execution requires --single-domain because staged workflows are single-domain")
    if args.workflow_checkpoint_store is not None and not args.workflow_execution:
        raise ValueError("--workflow-checkpoint-store requires --workflow-execution")
    if args.resume_workflow and args.workflow_checkpoint_store is None:
        raise ValueError("--resume-workflow requires --workflow-checkpoint-store")
    if args.workflow_max_stage_calls is not None and not 1 <= args.workflow_max_stage_calls <= 16:
        raise ValueError("--workflow-max-stage-calls must be between 1 and 16")
    workflow_checkpoint = (
        _load_workflow_checkpoint(args.workflow_checkpoint_store)
        if args.resume_workflow and args.workflow_checkpoint_store is not None
        else None
    )
    persisted_candidates = _persisted_candidate_args(args) if args.use_inventory else ()
    runtime, onboarding = _runtime_with_provider(args)
    session = onboarding.start_session(ttl_seconds=args.ttl_seconds)
    health_ledger = None
    learning_ledger = None
    execution_journal = None
    try:
        if args.health_store is not None:
            health_ledger = ProviderHealthLedger(args.health_store)
        if args.learning_store is not None:
            learning_ledger = SQLiteBrainLearningLedger(args.learning_store)
        if args.execution_store is not None:
            execution_journal = AutonomousExecutionJournal(args.execution_store)
        _collect_credentials(
            args,
            session,
            environ=environ,
            reader=reader,
        )
        descriptors = _discover_descriptors(runtime, args, session) if args.discover_models else ()
        if args.discover_models and not descriptors:
            raise ValueError("provider discovery returned no selectable models")
        candidates = (
            _candidate_args(args, descriptors)
            if args.discover_models
            else persisted_candidates
            if args.use_inventory
            else _candidate_args(args)
        )
        catalogue = ModelCatalogue(candidates)
        client = client_factory(
            command,
            cwd=args.mcp_cwd,
            timeout=args.mcp_timeout,
        )
        with client:
            agent_kwargs: dict[str, Any] = {"model_catalogue": catalogue}
            if learning_ledger is not None:
                agent_kwargs["ledger"] = learning_ledger
            if health_ledger is not None:
                agent_kwargs["health_ledger"] = health_ledger
            if execution_journal is not None:
                agent_kwargs["execution_journal"] = execution_journal
            agent = AutonomousAgent(_McpWorkspace(client), runtime, **agent_kwargs)
            provider_tools = (
                _mcp_provider_tools(client)
                if args.execution_mode in {"tool_loop", "mission"}
                else ()
            )
            common = {
                "task": args.task,
                "credentials": session,
                "model_candidates": candidates,
                "capability": args.capability,
                "required_model_capabilities": tuple(args.required_model_capability or ()),
                "execution_mode": args.execution_mode,
                "max_steps": args.max_steps,
                "requested_output_tokens": args.requested_output_tokens,
                "max_output_tokens": args.requested_output_tokens,
                "approve_provider_call": args.approve_provider_call,
                "approve_mission_dispatch": args.approve_mission_dispatch,
                "run_id": args.run_id,
                "execution_id": args.execution_id,
                "resume_execution": args.resume_execution,
            }
            if provider_tools:
                common["provider_tools"] = provider_tools
                common["tool_loop_options"] = {
                    "authorize_and_execute": _cli_tool_authorizer(
                        client,
                        approve_mission_dispatch=args.approve_mission_dispatch,
                    ),
                }
            if args.automatic:
                result = agent.run_auto(
                    **common,
                    learning_mode=args.learning_mode,
                    hints=tuple(args.hint or ()),
                    max_domains=args.max_domains,
                    allow_cross_domain=not args.single_domain,
                    semantic_routing=args.semantic_routing,
                    planning_mode=args.planning_mode,
                    planning_run_id=args.planning_run_id,
                    planning_max_output_tokens=args.planning_max_output_tokens,
                    workflow_execution=args.workflow_execution,
                    workflow_checkpoint=workflow_checkpoint,
                    workflow_retry_blocked=args.workflow_retry_blocked,
                    workflow_max_stage_calls=args.workflow_max_stage_calls,
                )
            else:
                if args.learning_mode == "online":
                    common["learn"] = True
                result = agent.run(**common, domain=args.domain)
        workflow_result_checkpoint = _workflow_checkpoint_from_result(result)
        workflow_persistence = {
            "configured": args.workflow_checkpoint_store is not None,
            "workflow_execution_requested": args.workflow_execution,
            "store": args.workflow_checkpoint_store,
            "resume_requested": args.resume_workflow,
            "checkpoint_loaded": workflow_checkpoint is not None,
            "checkpoint_persisted": False,
            "checkpoint_available": workflow_result_checkpoint is not None,
            "checkpoint_digest": None,
            "completed_stage_ids": [],
            "retention": "caller_owned_structured_stage_metadata; status_projection_excludes_structured_values",
        }
        if workflow_result_checkpoint is not None:
            workflow_persistence.update(
                {
                    "checkpoint_digest": workflow_result_checkpoint.checkpoint_digest,
                    "completed_stage_ids": list(workflow_result_checkpoint.completed_stage_ids),
                }
            )
            if args.workflow_checkpoint_store is not None:
                _persist_workflow_checkpoint(args.workflow_checkpoint_store, workflow_result_checkpoint)
                workflow_persistence["checkpoint_persisted"] = True

        execution_id = args.execution_id
        execution_state = None
        if execution_journal is not None:
            if execution_id is None:
                rows = execution_journal.events(limit=execution_journal.max_events)
                if rows and isinstance(rows[-1].get("event"), Mapping):
                    candidate_id = rows[-1]["event"].get("execution_id")
                    if isinstance(candidate_id, str):
                        execution_id = candidate_id
            if execution_id is not None:
                state = execution_journal.state(execution_id)
                execution_state = None if state is None else state.to_dict()
        return {
            "schema": CLI_SCHEMA,
            "command": "run",
            "routing_mode": "automatic" if args.automatic else "explicit_domain",
            "model_inventory": {
                "mode": (
                    "provider_discovery"
                    if args.discover_models
                    else "persisted_catalogue"
                    if args.use_inventory
                    else "caller_declared"
                ),
                "models": [descriptor.to_dict() for descriptor in descriptors],
                "model_count": len(descriptors),
                "candidates": [candidate.to_dict() for candidate in candidates]
                if args.use_inventory
                else [],
            },
            "result": result,
            "provider_status": runtime.provider_status(args.provider),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": args.approve_provider_call,
                "model_discovery_approved": args.approve_provider_call if args.discover_models else False,
                "mission_dispatch_approved": args.approve_mission_dispatch,
            },
            "state_persistence": {
                "health_store_configured": health_ledger is not None,
                "learning_store_configured": learning_ledger is not None,
                "learning_mode": args.learning_mode,
                "execution_store_configured": execution_journal is not None,
                "execution_id": execution_id,
                "resume_execution": args.resume_execution,
            },
            "execution": (
                {
                    "execution_id": execution_id,
                    "state": execution_state,
                    "retention": "metadata_only_hash_chained",
                }
                if execution_journal is not None
                else None
            ),
            "workflow": workflow_persistence,
            "secret_material": "never_returned",
        }
    finally:
        session.close()
        if learning_ledger is not None:
            learning_ledger.close()


def _parser() -> argparse.ArgumentParser:
    parser = _ArgumentParser(
        prog="aurora-agent",
        description="Operate the AURORA autonomous brain through a secret-safe process boundary.",
    )
    subparsers = parser.add_subparsers(
        dest="command",
        required=True,
        parser_class=_ArgumentParser,
    )

    subparsers.add_parser(
        "catalogue",
        help="show all reviewed autonomous domains, workflows, packs, and evaluators",
    )

    plan = subparsers.add_parser("evidence-plan", help="compile a provider-free evidence plan")
    plan.add_argument("--domain", action="append", help="domain to include; repeatable (default: all)")
    plan.add_argument("--available-evidence", action="append", default=[], help="known evidence label or digest")

    route = subparsers.add_parser("route", help="route a task without contacting a provider")
    route.add_argument("--task", required=True)
    route.add_argument("--hint", action="append", default=[], help="caller routing hint; repeatable")

    provider_parent = _ArgumentParser(add_help=False)
    provider_parent.add_argument("--provider", default="openai", help="provider identifier")
    provider_parent.add_argument("--base-url", help="provider base URL; required for custom providers")
    provider_parent.add_argument("--provider-path", default=None, help="custom OpenAI-compatible path")
    provider_parent.add_argument("--models-path", default=None, help="custom model inventory path")
    provider_parent.add_argument(
        "--local-model",
        default="local-model",
        help="model identifier exposed by the explicit credentialless local provider",
    )
    provider_parent.add_argument(
        "--local-response",
        default="Local provider completed the requested task.",
        help="bounded text returned by the explicit local provider",
    )
    provider_parent.add_argument(
        "--local-response-json",
        default=None,
        help="optional JSON object returned by the explicit local provider",
    )
    provider_parent.add_argument(
        "--local-response-sequence-json",
        default=None,
        help=(
            "optional bounded JSON-object response sequence for credentialless tool-loop "
            "verification"
        ),
    )

    subparsers.add_parser("provider-status", parents=[provider_parent], help="show redacted provider readiness")

    onboarding = subparsers.add_parser("onboard", parents=[provider_parent], help="collect one short-lived key and show redacted status")
    _add_credential_arguments(onboarding)

    discovery = subparsers.add_parser(
        "discover-models",
        parents=[provider_parent],
        help="discover a bounded, metadata-only provider model inventory",
    )
    discovery.add_argument("--model-limit", type=int, default=64, help="maximum inventory rows to inspect")
    discovery.add_argument("--approve-provider-call", action="store_true", help="authorize provider inventory discovery")
    _add_credential_arguments(discovery)

    refresh = subparsers.add_parser(
        "refresh-models",
        parents=[provider_parent],
        help="discover, reconcile, and optionally persist provider models with all-domain coverage",
    )
    refresh.add_argument("--model-limit", type=int, default=64, help="maximum provider inventory rows to inspect")
    refresh.add_argument("--inventory-store", default=None, help="optional metadata-only snapshot path")
    refresh.add_argument("--refresh-id", default=None, help="caller-owned bounded refresh identity")
    refresh.add_argument("--model-capability", action="append", default=[], help="caller-declared capability for every discovered model")
    refresh.add_argument("--context-window-tokens", type=int, default=_DEFAULT_CONTEXT_WINDOW)
    refresh.add_argument("--model-max-output-tokens", type=int, default=_DEFAULT_MAX_OUTPUT)
    refresh.add_argument("--quality", type=float, default=0.5)
    refresh.add_argument("--reliability", type=float, default=0.5)
    refresh.add_argument("--latency-ms", type=int, default=1_000)
    refresh.add_argument("--cost-per-million-tokens", type=int, default=0)
    refresh.add_argument("--raise-on-error", action="store_true", help="fail closed instead of returning a failed provider row")
    refresh.add_argument("--approve-provider-call", action="store_true", help="authorize provider inventory refresh")
    _add_credential_arguments(refresh)

    inventory_status = subparsers.add_parser(
        "inventory-status",
        help="read a persisted model inventory snapshot without contacting a provider",
    )
    inventory_status.add_argument("--inventory-store", required=True, help="metadata-only snapshot path")

    state_status = subparsers.add_parser(
        "state-status",
        help="read persisted provider health and bandit state without contacting a provider",
    )
    state_status.add_argument("--health-store", default=None, help="provider health JSONL path")
    state_status.add_argument("--learning-store", default=None, help="SQLite value-only learning ledger path")

    learning_status = subparsers.add_parser(
        "learning-status",
        help="inspect pending learning episodes and replay metadata without contacting a provider",
    )
    learning_status.add_argument("--learning-store", required=True, help="SQLite value-only learning ledger path")
    learning_status.add_argument("--episode-id", default=None, help="optionally project one episode identity")
    learning_status.add_argument("--limit", type=int, default=128, help="maximum pending/replay rows to return")

    execution_status = subparsers.add_parser(
        "execution-status",
        help="inspect hash-verified autonomous execution checkpoints without contacting a provider",
    )
    execution_status.add_argument("--execution-store", required=True, help="metadata-only execution journal path")
    execution_status.add_argument("--execution-id", default=None, help="optionally filter one execution identity")
    execution_status.add_argument("--limit", type=int, default=256, help="maximum execution transitions to return")

    workflow_status = subparsers.add_parser(
        "workflow-status",
        help="inspect a digest-verified staged workflow checkpoint without contacting a provider",
    )
    workflow_status.add_argument(
        "--workflow-checkpoint-store",
        required=True,
        help="caller-owned workflow checkpoint store",
    )

    batch_status = subparsers.add_parser(
        "batch-status",
        help="inspect a digest-verified batch checkpoint without contacting a provider",
    )
    batch_status.add_argument(
        "--batch-checkpoint-store",
        required=True,
        help="caller-owned metadata-only batch checkpoint store",
    )

    settle_learning = subparsers.add_parser(
        "settle-learning",
        help="settle one prevalidated value-only evaluator decision without a provider call",
    )
    settle_learning.add_argument("--learning-store", required=True, help="existing SQLite value-only learning ledger path")
    settle_learning.add_argument("--episode-id", required=True, help="pending episode identity")
    settle_learning.add_argument("--evaluator-id", required=True, help="bounded evaluator contract identity")
    settle_learning.add_argument("--evaluator-version", required=True, help="bounded evaluator contract version")
    settle_learning.add_argument("--reward", required=True, type=float, help="explicit evaluator reward in [-1, 1]")
    settle_learning.add_argument("--outcome", required=True, choices=("passed", "failed"))
    settle_learning.add_argument("--feedback-digest", default=None, help="optional SHA-256 evaluator feedback digest")
    settle_learning.add_argument("--failure-class", default=None, help="optional bounded failure class")
    settle_learning.add_argument("--evidence-digest", default=None, help="digest of separately retained evaluator evidence")
    settle_learning.add_argument("--mcp-command", required=True, help="MCP executable and arguments for the brain kernel; no shell is invoked")
    settle_learning.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    settle_learning.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)

    run = subparsers.add_parser("run", parents=[provider_parent], help="run one autonomous task through a caller-owned MCP workspace")
    run.add_argument("--mcp-command", required=True, help="MCP executable and arguments; no shell is invoked")
    run.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    run.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)
    run.add_argument("--task", required=True)
    run.add_argument("--domain", choices=AUTONOMOUS_DOMAINS, help="explicit domain; omit when using --automatic")
    run.add_argument("--automatic", action="store_true", help="route the task across the reviewed domain catalogue")
    run.add_argument("--hint", action="append", default=[], help="automatic-routing hint; repeatable")
    run.add_argument("--max-domains", type=int, default=3, help="maximum domains for automatic routing")
    run.add_argument("--single-domain", action="store_true", help="prevent automatic cross-domain fan-out")
    run.add_argument("--semantic-routing", action="store_true", help="use an approved provider call to refine routing")
    run.add_argument("--planning-mode", choices=("deterministic", "provider"), default="deterministic")
    run.add_argument("--planning-run-id", default=None)
    run.add_argument("--planning-max-output-tokens", type=int, default=1_024)
    run.add_argument("--model", action="append", default=[], help="candidate model; repeat to enable model selection or filter discovery")
    run.add_argument("--discover-models", action="store_true", help="discover selectable models through the approved provider inventory endpoint")
    run.add_argument("--use-inventory", action="store_true", help="rehydrate selectable candidates from --inventory-store without rediscovery")
    run.add_argument("--inventory-store", default=None, help="digest-bound metadata-only inventory store for --use-inventory")
    run.add_argument("--health-store", default=None, help="persist provider/model health observations across runs")
    run.add_argument("--learning-store", default=None, help="persist value-only online-learning state in SQLite")
    run.add_argument("--execution-store", default=None, help="persist hash-chained metadata-only execution checkpoints")
    run.add_argument("--execution-id", default=None, help="stable execution identity for persistence/resume")
    run.add_argument("--resume-execution", action="store_true", help="explicitly resume the named non-terminal execution")
    run.add_argument(
        "--workflow-execution",
        action="store_true",
        help="execute the selected automatic single-domain route as a checkpointable stage DAG",
    )
    run.add_argument(
        "--workflow-max-stage-calls",
        type=int,
        default=None,
        help="bound staged provider calls in this request; the checkpoint can continue later",
    )
    run.add_argument(
        "--workflow-retry-blocked",
        action="store_true",
        help="explicitly retry a blocked/proposed workflow stage from a resumed checkpoint",
    )
    run.add_argument(
        "--workflow-checkpoint-store",
        default=None,
        help="atomically persist the validated caller-owned workflow checkpoint",
    )
    run.add_argument(
        "--resume-workflow",
        action="store_true",
        help="load and explicitly resume the checkpoint at --workflow-checkpoint-store",
    )
    run.add_argument("--learning-mode", choices=("off", "online", "trajectory"), default="off", help="automatic route learning mode; rewards remain evaluator-gated")
    run.add_argument("--model-limit", type=int, default=64, help="maximum provider inventory rows to inspect when discovering")
    run.add_argument("--model-capability", action="append", default=[], help="declared capability for every model candidate")
    run.add_argument("--required-model-capability", action="append", default=[], help="capability required by this run")
    run.add_argument("--context-window-tokens", type=int, default=_DEFAULT_CONTEXT_WINDOW)
    run.add_argument("--model-max-output-tokens", type=int, default=_DEFAULT_MAX_OUTPUT)
    run.add_argument("--quality", type=float, default=0.5)
    run.add_argument("--reliability", type=float, default=0.5)
    run.add_argument("--latency-ms", type=int, default=1_000)
    run.add_argument("--cost-per-million-tokens", type=int, default=0)
    run.add_argument("--capability", default=None, help="domain capability label for the planner")
    run.add_argument("--execution-mode", choices=("provider", "tool_loop", "mission"), default="provider")
    run.add_argument("--max-steps", type=int, default=8)
    run.add_argument("--requested-output-tokens", type=int, default=2_048)
    run.add_argument("--run-id", default=None)
    run.add_argument("--approve-provider-call", action="store_true", help="authorize provider invocation")
    run.add_argument("--approve-mission-dispatch", action="store_true", help="authorize mission effects")
    _add_credential_arguments(run)

    batch_run = subparsers.add_parser(
        "batch-run",
        parents=[provider_parent],
        help="execute a bounded request file across explicit, automatic, or cross-domain routes",
    )
    batch_run.add_argument("--mcp-command", required=True, help="MCP executable and arguments; no shell is invoked")
    batch_run.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    batch_run.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)
    batch_run.add_argument("--requests-file", required=True, help="JSON request file validated before provider access")
    batch_run.add_argument("--job-id", required=True, help="stable batch identity for checkpoint/resume")
    batch_run.add_argument("--batch-mode", choices=("domain", "auto", "cross_domain"), default=None, help="optional mode assertion; the request file remains authoritative")
    batch_run.add_argument("--max-parallelism", type=int, default=4, help="bounded concurrent request count")
    batch_run.add_argument("--stop-on-error", action="store_true", help="omit remaining requests after the first failed/refused item")
    batch_run.add_argument("--single-domain", action="store_true", help="keep automatic requests on one reviewed domain")
    batch_run.add_argument("--batch-checkpoint-store", default=None, help="atomic metadata-only batch checkpoint path")
    batch_run.add_argument("--batch-result-manifest", default=None, help="status-only manifest used to rehydrate completed independent items")
    batch_run.add_argument("--resume-batch", action="store_true", help="explicitly resume the existing batch checkpoint")
    batch_run.add_argument("--model", action="append", default=[], help="shared candidate model; repeatable")
    batch_run.add_argument("--discover-models", action="store_true", help="discover selectable models through the approved provider inventory endpoint")
    batch_run.add_argument("--use-inventory", action="store_true", help="rehydrate selectable candidates from --inventory-store")
    batch_run.add_argument("--inventory-store", default=None, help="digest-bound metadata-only inventory store")
    batch_run.add_argument("--health-store", default=None, help="persist provider/model health observations")
    batch_run.add_argument("--learning-store", default=None, help="persist value-only online-learning state")
    batch_run.add_argument("--execution-store", default=None, help="persist hash-chained metadata-only execution checkpoints")
    batch_run.add_argument("--execution-mode", choices=("provider", "tool_loop", "mission"), default=None)
    batch_run.add_argument("--max-steps", type=int, default=None)
    batch_run.add_argument("--requested-output-tokens", type=int, default=None)
    batch_run.add_argument("--learning-mode", choices=("off", "online", "trajectory"), default=None, help="automatic route learning mode")
    batch_run.add_argument("--workflow-execution", action="store_true", help="enable checkpointable workflow execution for automatic requests")
    batch_run.add_argument("--workflow-max-stage-calls", type=int, default=None)
    batch_run.add_argument("--workflow-retry-blocked", action="store_true")
    batch_run.add_argument("--model-limit", type=int, default=64)
    batch_run.add_argument("--model-capability", action="append", default=[])
    batch_run.add_argument("--context-window-tokens", type=int, default=_DEFAULT_CONTEXT_WINDOW)
    batch_run.add_argument("--model-max-output-tokens", type=int, default=_DEFAULT_MAX_OUTPUT)
    batch_run.add_argument("--quality", type=float, default=0.5)
    batch_run.add_argument("--reliability", type=float, default=0.5)
    batch_run.add_argument("--latency-ms", type=int, default=1_000)
    batch_run.add_argument("--cost-per-million-tokens", type=int, default=0)
    batch_run.add_argument("--approve-provider-call", action="store_true", help="authorize provider invocation")
    batch_run.add_argument("--approve-mission-dispatch", action="store_true", help="authorize mission effects")
    _add_credential_arguments(batch_run)
    return parser


def _add_credential_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--credential-source",
        choices=("prompt", "environment"),
        default="prompt",
        help="where to obtain the key; prompt input is hidden and never echoed",
    )
    parser.add_argument("--credential-env", default=None, help="environment variable name when using environment source")
    parser.add_argument("--ttl-seconds", type=float, default=900.0)


def main(
    argv: Sequence[str] | None = None,
    *,
    environ: Mapping[str, str] | None = None,
    reader: Callable[[str], str] | None = None,
    writer: TextIO | None = None,
    error_writer: TextIO | None = None,
    client_factory: Callable[..., Client] = Client,
) -> int:
    """Run the CLI and return a process exit code.

    ``reader``/``environ``/``client_factory`` are explicit seams for tests and host UIs.  They
    do not change the production posture: the default reader is ``getpass.getpass`` and the
    default MCP client uses ``shell=False``.
    """

    out = writer or sys.stdout
    errors = error_writer or sys.stderr
    try:
        args = _parser().parse_args(None if argv is None else list(argv))
        env = os.environ if environ is None else environ
        if args.command == "catalogue":
            payload = _catalogue()
        elif args.command == "evidence-plan":
            payload = _evidence_plan(args)
        elif args.command == "route":
            payload = _route(args)
        elif args.command == "provider-status":
            payload = _provider_status(args)
        elif args.command == "onboard":
            payload = _onboard(args, environ=env, reader=reader)
        elif args.command == "discover-models":
            payload = _discover_models(args, environ=env, reader=reader)
        elif args.command == "refresh-models":
            payload = _refresh_models(args, environ=env, reader=reader)
        elif args.command == "inventory-status":
            payload = _inventory_status(args)
        elif args.command == "state-status":
            payload = _state_status(args)
        elif args.command == "learning-status":
            payload = _learning_status(args)
        elif args.command == "execution-status":
            payload = _execution_status(args)
        elif args.command == "workflow-status":
            payload = _workflow_status(args)
        elif args.command == "batch-status":
            payload = _batch_status(args)
        elif args.command == "settle-learning":
            payload = _settle_learning(args, client_factory=client_factory)
        elif args.command == "run":
            payload = _run(args, environ=env, reader=reader, client_factory=client_factory)
        elif args.command == "batch-run":
            payload = _batch_run(args, environ=env, reader=reader, client_factory=client_factory)
        else:  # pragma: no cover - argparse enforces the command set
            raise ValueError("unknown command")
        _write_json(out, payload)
        return 0
    except (KeyboardInterrupt, EOFError):
        errors.write("aurora-agent: interrupted while collecting input\n")
        return 130
    except _CliArgumentError:
        errors.write("aurora-agent: invalid command-line arguments\n")
        return 2
    except (ValueError, SdkError, OSError) as error:
        # Do not echo exception text at this boundary. Provider/MCP implementations are
        # intentionally defensive, but an operator-facing process should fail closed even if a
        # future adapter includes remote or credential-shaped text in an exception message.
        errors.write(f"aurora-agent: {type(error).__name__}: command failed\n")
        return 2
    except Exception as error:  # pragma: no cover - final process boundary
        errors.write(f"aurora-agent: {type(error).__name__}: command failed\n")
        return 2


__all__ = ["CLI_SCHEMA", "main"]
