"""Operator-facing command line boundary for the autonomous SDK.

The SDK is intentionally embeddable, but a useful autonomous system also needs a small,
well-defined process boundary.  This module provides that boundary without moving secrets into
the brain or into MCP:

* ``catalogue`` and ``evidence-plan`` are provider-free inspection commands;
* ``route`` exposes deterministic routing evidence without invoking a model;
* ``provider-status``, ``onboard``, and the inventory commands implement the redacted BYOK and
  model-lifecycle boundaries; and
* ``state-status`` and ``learning-status`` inspect persisted health/learning metadata, while
  ``settle-learning`` accepts only a bounded evaluator decision for a restart-safe settlement; and
* ``run`` connects to a caller-owned MCP workspace, collects one short-lived credential, lets the
  existing autonomous planner select a model, and requires explicit provider/mission approval.

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
from typing import Any, Callable, Mapping, Sequence, TextIO

from .autonomy import AUTONOMOUS_DOMAINS, AutonomousAgent
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
    anthropic_provider,
    openai_compatible_provider,
    openai_provider,
)


CLI_SCHEMA = "aurora-autonomous-cli/0.1"
_DEFAULT_CONTEXT_WINDOW = 128_000
_DEFAULT_MAX_OUTPUT = 4_096
_DEFAULT_TIMEOUT = 30.0
_LOCAL_PROVIDER_NAMES = frozenset({"local", "in_memory"})


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
        if local_response_json is not None:
            try:
                parsed_response = json.loads(local_response_json)
            except (TypeError, json.JSONDecodeError) as error:
                raise ValueError("--local-response-json must be valid JSON") from error
            if not isinstance(parsed_response, Mapping):
                raise ValueError("--local-response-json must be a JSON object")
        else:
            parsed_response = None

        def local_handler(_request: Any) -> Mapping[str, Any]:
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
                )
            else:
                if args.learning_mode == "online":
                    common["learn"] = True
                result = agent.run(**common, domain=args.domain)
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

    status = subparsers.add_parser("provider-status", parents=[provider_parent], help="show redacted provider readiness")

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
        elif args.command == "settle-learning":
            payload = _settle_learning(args, client_factory=client_factory)
        elif args.command == "run":
            payload = _run(args, environ=env, reader=reader, client_factory=client_factory)
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
