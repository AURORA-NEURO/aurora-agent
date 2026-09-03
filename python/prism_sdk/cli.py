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
  Both execution commands can additionally activate the SDK's exact curated read-only domain
  bindings, so the process boundary uses the same registry, activation, and capability narrowing
  path as an embedding application.
* ``grounded-portfolio`` runs the source-separated real-glioma and PubMed research loops through
  Ollama or an explicit in-memory fixture, with an atomic digest-bound ledger for restart/resume;
  opt-in ``--refresh-*`` flags can acquire current public snapshots without a credential.
* ``grounded-autopilot`` routes a free-text neurosurgical research question before any model call,
  gates the required real snapshot, and then invokes the same bounded source-separated loops;
  refreshes are explicit, allow-listed, and recorded as receipts.
* ``refresh-public-literature`` refreshes six bounded PubMed lanes without a credential and
  atomically installs the candidate only after local provenance/hash validation.
* ``refresh-real-glioma`` refreshes aggregate ClinicalTrials.gov, NCI GDC, cBioPortal, NCI PDQ,
  and PubMed metadata without a credential and atomically installs a validated population bundle.

The command line parser deliberately has no API-key, token, header, or secret argument.  Keys
are accepted only through the existing no-echo prompt or an explicitly named environment
variable, then revoked when the command exits.  MCP commands are passed as argv after parsing;
no shell is ever started by this boundary.
"""

from __future__ import annotations

import argparse
import getpass
import hashlib
import json
import os
import shlex
import sys
import tempfile
import threading
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence, TextIO

from .authoring import content_digest
from .autonomous_action_admission_controller import AutonomousActionAdmissionController
from .autonomous_action_admission_persistence import (
    AutonomousActionAdmissionPersistenceCoordinator,
    InMemoryAutonomousActionAdmissionLedger,
    TransactionalJsonAutonomousActionAdmissionSnapshotPersistence,
)
from .autonomous_action_plan import AutonomousActionPlan
from .autonomous_task_decision import AUTONOMOUS_TASK_DECISION_APPROVALS
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
from .autonomous_launch_admission import (
    MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES,
    authorize_autonomous_launch_domains,
    validate_autonomous_launch_admission,
)
from .autonomous_model_inventory import AutonomousModelInventoryStore
from .autonomy_onboarding import (
    AutonomousCapabilityActivation,
    AutonomousCapabilityActivationStore,
)
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
    ollama_provider,
    openai_compatible_provider,
    openai_provider,
)
from .memory import BrainEpisodicMemory
from .neurosurgery import LocalNeurosurgicalAgent
from .public_literature_refresh import atomic_refresh_neurosurgical_public_literature
from .real_data_refresh import (
    DEFAULT_GDC_PROJECT_IDS,
    DEFAULT_PORTAL_STUDY_IDS,
    DEFAULT_PUBMED_SOURCE_ID,
    DEFAULT_PUBMED_TERM,
    atomic_refresh_real_glioma_data,
)
from .tooling import ToolCatalogue


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
_MAX_DOMAIN_TOOL_BINDING_FILE_BYTES = 512_000
_MAX_EVIDENCE_FILE_BYTES = 128_000
_MAX_ACTION_PLAN_FILE_BYTES = 4_000_000
_MAX_ACTION_ADMISSION_STORE_BYTES = 4_000_000
_MAX_GROUNDED_PORTFOLIO_BUNDLE_BYTES = 2_000_000
_MAX_GROUNDED_PORTFOLIO_STORE_BYTES = 8_000_000
_MAX_GROUNDED_QUERY_FILE_BYTES = 64_000
GROUNDED_PORTFOLIO_CLI_SCHEMA = "aurora-grounded-portfolio-cli/0.1"
GROUNDED_INTAKE_CLI_SCHEMA = "aurora-grounded-intake-cli/0.1"
GROUNDED_INTAKE_STORE_SCHEMA = "aurora-grounded-intake-store/0.1"
PUBLIC_LITERATURE_REFRESH_CLI_SCHEMA = "aurora-public-literature-refresh-cli/0.1"
REAL_DATA_REFRESH_CLI_SCHEMA = "aurora-real-data-refresh-cli/0.1"
CLI_DOMAIN_TOOL_BINDINGS_SCHEMA = "aurora-cli-domain-tool-bindings/0.1"


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
            # Keep the process boundary portable across Windows code pages.  JSON consumers
            # decode these escapes back to the original Unicode source metadata, while a local
            # console that is not UTF-8 capable cannot crash the otherwise valid handoff.
            ensure_ascii=True,
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
    if provider == "ollama":
        return ollama_provider(base_url=base_url or "http://127.0.0.1:11434/v1")
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
            for item in parsed_sequence:
                raw_calls = item.get("tool_calls", [])
                if (
                    not isinstance(raw_calls, list)
                    or len(raw_calls) > _MAX_MCP_PROVIDER_TOOLS
                    or any(not isinstance(call, Mapping) for call in raw_calls)
                ):
                    raise ValueError(
                        "--local-response-sequence-json tool_calls must be a bounded array of objects"
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


def _mcp_provider_tools(
    client: Client,
    *,
    allow_tools: Sequence[str] = (),
    deny_tools: Sequence[str] = (),
) -> tuple[tuple[ProviderTool, ...], ToolCatalogue]:
    """Convert the live MCP catalogue into a bounded, schema-checked tool surface."""

    schemas = client.list_tools()
    if len(schemas) > _MAX_MCP_PROVIDER_TOOLS:
        raise ValueError(
            "MCP workspace advertises more than "
            f"{_MAX_MCP_PROVIDER_TOOLS} model-visible tools"
        )
    catalogue = ToolCatalogue.from_definitions(schemas)
    names = {definition.name for definition in catalogue.definitions}
    allowed = set(allow_tools)
    denied = set(deny_tools)
    if any(not isinstance(name, str) or not name.strip() for name in (*allow_tools, *deny_tools)):
        raise ValueError("MCP tool policy names must be non-empty strings")
    unknown_allowed = sorted(allowed.difference(names))
    unknown_denied = sorted(denied.difference(names))
    if unknown_allowed:
        raise ValueError("MCP allowlist names absent from tools/list: " + ", ".join(unknown_allowed))
    if unknown_denied:
        raise ValueError("MCP denylist names absent from tools/list: " + ", ".join(unknown_denied))
    selected_names = names if not allowed else allowed
    selected_names.difference_update(denied)
    selected_schemas = tuple(
        schema
        for schema in schemas
        if isinstance(schema, Mapping) and schema.get("name") in selected_names
    )
    selected_catalogue = ToolCatalogue.from_definitions(selected_schemas)
    tools: list[ProviderTool] = []
    seen: set[str] = set()
    for schema in selected_schemas:
        tool = ProviderTool.from_mcp_schema(schema)
        if tool.name in seen:
            raise ValueError("MCP workspace advertises duplicate tool names")
        seen.add(tool.name)
        tools.append(tool)
    return tuple(tools), selected_catalogue


def _cli_tool_authorizer(
    client: Client,
    *,
    catalogue: ToolCatalogue,
    approve_mission_dispatch: bool,
    allowed_tools: Sequence[str] | None = None,
    read_only_tools: Sequence[str] = (),
) -> Callable[[tuple[ProviderToolCall, ...]], Sequence[ProviderToolResult]]:
    """Build the CLI's explicit approval boundary for provider-requested MCP calls."""

    allowed = None if allowed_tools is None else frozenset(allowed_tools)
    read_only = frozenset(read_only_tools)

    def authorize(calls: tuple[ProviderToolCall, ...]) -> Sequence[ProviderToolResult]:
        if len({call.call_id for call in calls}) != len(calls):
            return tuple(
                ProviderToolResult(
                    call.call_id,
                    {"ok": False, "status": "duplicate_tool_call_id"},
                    approved=False,
                    is_error=True,
                )
                for call in calls
            )
        plans = []
        for call in calls:
            if allowed is not None and call.name not in allowed:
                return tuple(
                    ProviderToolResult(
                        pending.call_id,
                        {"ok": False, "status": "domain_tool_not_activated"},
                        approved=False,
                        is_error=True,
                    )
                    for pending in calls
                )
            try:
                plans.append(catalogue.plan(call.name, call.arguments))
            except Exception:
                return tuple(
                    ProviderToolResult(
                        pending.call_id,
                        {"ok": False, "status": "tool_arguments_rejected"},
                        approved=False,
                        is_error=True,
                    )
                    for pending in calls
                )
        if not approve_mission_dispatch and any(call.name not in read_only for call in calls):
            return tuple(
                ProviderToolResult(
                    call.call_id,
                    {
                        "ok": False,
                        "status": "approval_required",
                        "authorization": "operator",
                    },
                    approved=False,
                    is_error=True,
                )
                for call in calls
            )
        results: list[ProviderToolResult] = []
        for call, plan in zip(calls, plans):
            try:
                value = client.call_tool(call.name, plan.to_mcp_arguments()).require_ok()
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


def _domain_tool_domains(args: argparse.Namespace, *, batch: bool = False) -> tuple[str, ...]:
    """Resolve the explicit scope used by the curated live-tool activation planner."""

    requested = tuple(getattr(args, "domain_tool_domain", ()) or ())
    if requested:
        return _domains(requested)
    if batch or getattr(args, "automatic", False):
        return _domains(None)
    domain = getattr(args, "domain", None)
    if domain in AUTONOMOUS_DOMAINS:
        return (domain,)
    return _domains(None)


def _activate_domain_tools(
    agent: AutonomousAgent,
    catalogue: ToolCatalogue,
    *,
    domains: Sequence[str],
    activate: bool,
    approved_tools: Sequence[str],
) -> dict[str, Any] | None:
    """Plan and optionally activate exact curated read-only bindings from ``tools/list``.

    The planner is deliberately kept in the autonomous SDK rather than reimplemented in the
    process boundary.  This helper only chooses the operator-requested activation subset and
    projects redacted evidence for the CLI response.  Registration never grants effect
    authority; mission dispatch remains independently gated by ``--approve-mission-dispatch``.
    """

    if not activate and not approved_tools:
        return None
    plan = agent.plan_workspace_tool_bindings(catalogue, domains=tuple(domains))
    proposed = plan.get("proposed_bindings")
    if not isinstance(proposed, Mapping):
        raise ValueError("domain tool binding plan returned malformed proposed_bindings")
    requested = tuple(approved_tools) if approved_tools else tuple(sorted(proposed))
    if len(set(requested)) != len(requested):
        raise ValueError("--approve-domain-tool names must be unique")
    unknown = sorted(set(requested).difference(proposed))
    if unknown:
        raise ValueError(
            "domain tool activation accepts only exact curated read-only proposals: "
            + ", ".join(unknown)
        )
    registered: list[dict[str, Any]] = []
    if requested:
        registered = agent.register_workspace_bindings_from_plan(
            plan,
            requested,
            catalogue=catalogue,
        )
    activation = agent.activation_state()
    registry = agent.tool_registry
    registered_names = sorted(
        row["name"] for row in registered if isinstance(row, Mapping) and isinstance(row.get("name"), str)
    )
    descriptor = {
        "requested": True,
        "domains": list(domains),
        "plan_digest": activation.get("plan_digest"),
        "catalogue_digest": plan.get("catalogue_digest"),
        "profile_digest": plan.get("profile_digest"),
        "available_curated_count": len(plan.get("available_curated_tools", ())),
        "proposed_count": len(proposed),
        "review_required_count": len(plan.get("review_required_tools", ())),
        "unclassified_count": len(plan.get("unclassified_tools", ())),
        "missing_curated_count": len(plan.get("missing_curated_tools", ())),
        "approved_tools": sorted(requested),
        "registered_tools": registered_names,
        "registry_digest": None if registry is None else registry.digest,
        "activation_status": activation.get("status"),
        "activation_revision": activation.get("revision"),
        "activation_authority": "activation_approved_tools_only",
        "effect_authority": "separate_operator_mission_dispatch_required",
        "retention": "plan_and_registry_digests; tool names and safety counts only",
    }
    return descriptor


def _load_activation_store(
    args: argparse.Namespace,
) -> tuple[AutonomousCapabilityActivationStore | None, Any | None, bool]:
    """Load an explicitly requested redacted activation snapshot, never credentials."""

    path = getattr(args, "activation_store", None)
    resume = bool(getattr(args, "resume_activation", False))
    if resume and not path:
        raise ValueError("--resume-activation requires --activation-store")
    if path is None:
        return None, None, False
    store = AutonomousCapabilityActivationStore(path)
    if not resume:
        return store, None, False
    state = store.load()
    if state is None:
        raise ValueError("--resume-activation requires an existing activation snapshot")
    return store, state, True


def _activation_domains(state: Mapping[str, Any]) -> tuple[str, ...]:
    """Recover the exact reviewed domain scope from a redacted activation snapshot."""

    rows = state.get("domain_statuses", ())
    if isinstance(rows, Sequence) and not isinstance(rows, (str, bytes)):
        domains = tuple(
            row.get("domain")
            for row in rows
            if isinstance(row, Mapping) and isinstance(row.get("domain"), str)
        )
        if domains:
            return _domains(domains)
    return _domains(None)


def _rehydrate_domain_tools(
    agent: AutonomousAgent,
    catalogue: ToolCatalogue,
    *,
    previous_state: Mapping[str, Any],
) -> dict[str, Any]:
    """Revalidate and re-register approved bindings after a process restart.

    The activation file contains only approved names and digests, never the original schemas.
    Rehydration therefore requires a fresh live catalogue and recomputes the reviewed plan. A
    catalogue/profile change clears approvals in the SDK activation state before this helper can
    register anything, making drift fail closed without replaying a stale schema.
    """

    domains = _activation_domains(previous_state)
    plan = agent.plan_workspace_tool_bindings(catalogue, domains=domains)
    activation = agent.activation_state()
    approved = tuple(activation.get("approved_tools", ()))
    proposed = plan.get("proposed_bindings")
    if not isinstance(proposed, Mapping):
        raise ValueError("activation rehydration received malformed proposed_bindings")
    missing = sorted(set(approved).difference(proposed))
    if missing:
        raise ValueError("activation approvals could not be revalidated")
    registered: list[dict[str, Any]] = []
    if approved:
        registered = agent.register_workspace_bindings_from_plan(
            plan,
            approved,
            catalogue=catalogue,
        )
    final_state = agent.activation_state()
    registry = agent.tool_registry
    return {
        "requested": True,
        "mode": "resumed",
        "domains": list(domains),
        "plan_digest": final_state.get("plan_digest"),
        "catalogue_digest": final_state.get("catalogue_digest"),
        "profile_digest": final_state.get("profile_digest"),
        "approved_tools": sorted(approved),
        "registered_tools": sorted(
            row["name"]
            for row in registered
            if isinstance(row, Mapping) and isinstance(row.get("name"), str)
        ),
        "registry_digest": None if registry is None else registry.digest,
        "activation_status": final_state.get("status"),
        "activation_revision": final_state.get("revision"),
        "activation_authority": "activation_approved_tools_only",
        "effect_authority": "separate_operator_mission_dispatch_required",
        "retention": "revalidated_digests; approved names and status only",
    }


def _activation_persistence_projection(
    store: AutonomousCapabilityActivationStore | None,
    *,
    resumed: bool,
    persisted: bool,
    state: Mapping[str, Any] | None,
) -> dict[str, Any]:
    """Project activation persistence without exposing state beyond its redacted digest."""

    return {
        "configured": store is not None,
        "store": None if store is None else str(store.path),
        "resumed": resumed,
        "persisted": persisted,
        "state_digest": None if state is None else state.get("state_digest"),
        "status": None if state is None else state.get("status"),
        "retention": "activation_digests_and_status_only; credentials_and_handles_never_persisted",
    }


def _registered_tool_posture(agent: AutonomousAgent) -> frozenset[str]:
    """Return the names of registered read-only tools for the CLI approval boundary."""

    registry = agent.tool_registry
    if registry is None:
        return frozenset()
    return frozenset(tool.name for tool in registry.tools_for() if tool.read_only)


def _load_domain_tool_bindings_file(
    path: str,
) -> tuple[dict[str, Mapping[str, Any]], str]:
    """Load a strict caller-owned domain binding policy without accepting payload values."""

    if not isinstance(path, str) or not path.strip():
        raise ValueError("domain tool bindings file path must be non-empty")
    try:
        raw = Path(path).read_bytes()
    except OSError as error:
        raise ValueError("domain tool bindings file could not be read") from error
    if len(raw) > _MAX_DOMAIN_TOOL_BINDING_FILE_BYTES:
        raise ValueError("domain tool bindings file exceeds its bounded size")
    try:
        document = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("domain tool bindings file must contain valid UTF-8 JSON") from error
    if not isinstance(document, Mapping) or document.get("schema") != CLI_DOMAIN_TOOL_BINDINGS_SCHEMA:
        raise ValueError("domain tool bindings file schema is invalid")
    bindings = document.get("bindings")
    if not isinstance(bindings, Mapping) or not bindings:
        raise ValueError("domain tool bindings file requires a non-empty bindings object")
    if len(bindings) > _MAX_MCP_PROVIDER_TOOLS:
        raise ValueError("domain tool bindings file contains too many bindings")
    allowed_fields = {
        "name",
        "tool",
        "domains",
        "capability",
        "risk_class",
        "read_only",
        "approval_required",
    }
    normalized: dict[str, Mapping[str, Any]] = {}
    for key, value in bindings.items():
        if not isinstance(key, str) or not key.strip():
            raise ValueError("domain tool binding keys must be non-empty strings")
        if not isinstance(value, Mapping):
            raise ValueError(f"domain tool binding {key!r} must be an object")
        unknown = sorted(set(value).difference(allowed_fields))
        if unknown:
            raise ValueError(f"domain tool binding {key!r} contains unsupported fields")
        row = dict(value)
        row.setdefault("name", key)
        if row.get("name") != key or (
            row.get("tool") is not None and row.get("tool") != key
        ):
            raise ValueError(f"domain tool binding {key!r} does not match its name")
        domains = row.get("domains")
        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)) or not domains:
            raise ValueError(f"domain tool binding {key!r} requires domains")
        if any(not isinstance(domain, str) for domain in domains):
            raise ValueError(f"domain tool binding {key!r} contains malformed domains")
        unknown_domains = sorted(set(domains).difference(AUTONOMOUS_DOMAINS))
        if unknown_domains:
            raise ValueError(f"domain tool binding {key!r} contains unsupported domains")
        normalized[key] = row
    return normalized, content_digest(normalized)


def _register_domain_tool_bindings_file(
    agent: AutonomousAgent,
    catalogue: ToolCatalogue,
    path: str,
    *,
    approve_mission_dispatch: bool = False,
) -> dict[str, Any]:
    """Register explicit application policy for custom live MCP tools."""

    bindings, file_digest = _load_domain_tool_bindings_file(path)
    registered = agent.register_workspace_tools(
        bindings,
        catalogue=catalogue,
        require_all=False,
    )
    runtime = agent.tool_runtime
    if runtime is not None:
        # AutonomousAgent intentionally installs its registered runtime ahead of caller-supplied
        # raw loop callbacks. Bind the CLI's explicit operator decision at that authoritative
        # runtime boundary so effectful custom tools remain denied by default and become runnable
        # only for this invocation when the separate mission gate is present.
        prior_approval = runtime.approve
        runtime.approve = (
            (lambda tool, call: bool(approve_mission_dispatch) and (
                prior_approval is None or bool(prior_approval(tool, call))
            ))
        )
    read_only_count = sum(1 for row in bindings.values() if row.get("read_only", True) is True)
    effectful_count = len(bindings) - read_only_count
    state = agent.activation_state()
    registry = agent.tool_registry
    return {
        "requested": True,
        "mode": "explicit_file",
        "file_digest": file_digest,
        "binding_count": len(bindings),
        "domains": sorted({
            domain
            for row in bindings.values()
            for domain in row.get("domains", ())
            if isinstance(domain, str)
        }),
        "registered_tools": sorted(
            row["name"]
            for row in registered
            if isinstance(row, Mapping) and isinstance(row.get("name"), str)
        ),
        "read_only_count": read_only_count,
        "effectful_count": effectful_count,
        "registry_digest": None if registry is None else registry.digest,
        "activation_status": state.get("status"),
        "activation_authority": "caller_supplied_binding_metadata; execution_separately_gated",
        "effect_authority": "separate_operator_mission_dispatch_required",
        "retention": "file_digest_registry_digest_tool_names_and_risk_counts_only",
    }


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
        # POSIX mode treats Windows path separators as escape characters (for example,
        # ``C:\\Users\\...`` becomes ``C:Users...``).  Use the native Windows tokenization
        # when this process runs there, then remove only balanced command-string quotes; the
        # resulting argv is still passed to ``subprocess`` with ``shell=False``.
        if os.name == "nt":
            raw_command = shlex.split(value, posix=False)
            command = tuple(
                token[1:-1]
                if len(token) >= 2 and token[0] == token[-1] and token[0] in {"'", '"'}
                else token
                for token in raw_command
            )
        else:
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


def _action_plan(args: argparse.Namespace) -> dict[str, Any]:
    """Compile one or all explicit-domain action plans without provider access."""

    if args.all_domains and args.domain:
        raise ValueError("action-plan --all-domains cannot be combined with --domain")
    agent = AutonomousAgent(_OfflineWorkspace(), LLMRuntime())
    requested_domains: tuple[str | None, ...]
    if args.all_domains:
        requested_domains = tuple(AUTONOMOUS_DOMAINS)
    elif args.domain:
        requested_domains = tuple(args.domain)
    else:
        requested_domains = (None,)
    route_options = {
        "hints": tuple(args.hint or ()),
        "allow_cross_domain": not args.single_domain,
        "max_domains": args.max_domains,
    }
    plans: list[dict[str, Any]] = []
    for domain in requested_domains:
        plan = agent.action_plan(task=args.task, domain=domain, **route_options)
        if '"task":' in json.dumps(plan, ensure_ascii=False):
            raise ValueError("action-plan unexpectedly retained task material")
        plans.append(plan)
    return {
        "schema": CLI_SCHEMA,
        "command": "action-plan",
        "plans": plans,
        "plan_count": len(plans),
        "requested_domains": [domain for domain in requested_domains if domain is not None],
        "automatic": requested_domains == (None,),
        "authorization": "planning_evidence_only;no_provider_source_tool_evaluator_credential_or_effect_authority",
        "retention": "metadata_only;task_prompt_and_runtime_values_not_returned",
        "secret_material": "never_returned",
    }


class _ActionAdmissionFileStore:
    """Atomic text store used by the CLI's caller-owned action admission ledger."""

    def __init__(self, path_value: str) -> None:
        self.path = Path(path_value)

    def read(self) -> str | None:
        if not self.path.exists():
            return None
        if not self.path.is_file() or self.path.stat().st_size > _MAX_ACTION_ADMISSION_STORE_BYTES:
            raise ValueError("action admission store is outside its bounded file contract")
        try:
            return self.path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            raise ValueError("action admission store is unreadable") from error

    def write(self, value: str) -> None:
        self._atomic_write(value)

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        current = self.read()
        if current is None:
            if expected_snapshot_digest is not None:
                return False
        else:
            try:
                current_digest = json.loads(current).get("snapshot_digest")
            except (TypeError, ValueError, AttributeError):
                return False
            if current_digest != expected_snapshot_digest:
                return False
        self._atomic_write(value)
        return True

    def _atomic_write(self, value: str) -> None:
        if not isinstance(value, str) or len(value.encode("utf-8")) > _MAX_ACTION_ADMISSION_STORE_BYTES:
            raise ValueError("action admission store exceeds its bounded size")
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
                temporary.write(value)
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


def _action_admission_context(
    path_value: str,
) -> tuple[AutonomousActionAdmissionController, AutonomousActionAdmissionPersistenceCoordinator]:
    store = _ActionAdmissionFileStore(path_value)
    persistence = TransactionalJsonAutonomousActionAdmissionSnapshotPersistence(store)
    ledger = InMemoryAutonomousActionAdmissionLedger()
    coordinator = AutonomousActionAdmissionPersistenceCoordinator(ledger, persistence)
    coordinator.restore()
    return AutonomousActionAdmissionController(ledger), coordinator


def _action_plan_from_file(path_value: str) -> AutonomousActionPlan:
    path = Path(path_value)
    if not path.exists() or not path.is_file() or path.stat().st_size > _MAX_ACTION_PLAN_FILE_BYTES:
        raise ValueError("action plan file is outside its bounded file contract")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("action plan file is unreadable") from error
    if isinstance(raw, Mapping) and isinstance(raw.get("plan"), Mapping):
        raw = raw["plan"]
    if not isinstance(raw, Mapping):
        raise ValueError("action plan file must contain a serialized plan")
    try:
        plan = AutonomousActionPlan.from_dict(raw)
    except Exception as error:
        raise ValueError("action plan file is not a valid metadata-only plan") from error
    if '"task":' in json.dumps(plan.to_dict(), ensure_ascii=False):
        raise ValueError("action plan file retained task material")
    return plan


def _action_admission_status(args: argparse.Namespace) -> dict[str, Any]:
    controller, coordinator = _action_admission_context(args.admission_store)
    queue = controller.queue()
    return {
        "schema": CLI_SCHEMA,
        "command": "action-admission-status",
        "admission_store": args.admission_store,
        "queue": queue,
        "snapshot_digest": coordinator.expected_snapshot_digest,
        "authorization": "metadata_read_only;no_provider_source_tool_evaluator_credential_or_effect_authority",
        "retention": "metadata_only;task_prompt_and_runtime_values_not_returned",
        "secret_material": "never_returned",
    }


def _action_admission_submit(args: argparse.Namespace) -> dict[str, Any]:
    controller, coordinator = _action_admission_context(args.admission_store)
    plan = _action_plan_from_file(args.plan_file)
    row = controller.submit(args.action_id, plan)
    snapshot = coordinator.flush()
    return {
        "schema": CLI_SCHEMA,
        "command": "action-admission-submit",
        "admission_store": args.admission_store,
        "row": row,
        "snapshot_digest": snapshot["snapshot_digest"],
        "queue": controller.queue(),
        "authorization": "submission_only;review_and_downstream_provider_source_tool_evaluator_credential_and_effect_gates_remain_required",
        "retention": "metadata_only;task_prompt_and_runtime_values_not_returned",
        "secret_material": "never_returned",
    }


def _action_review_approvals(args: argparse.Namespace) -> dict[str, bool]:
    approved = tuple(args.approve_gate or ())
    denied = tuple(args.deny_gate or ())
    overlap = sorted(set(approved).intersection(denied))
    if overlap:
        raise ValueError("the same approval gate cannot be both approved and denied")
    return {**{gate: True for gate in approved}, **{gate: False for gate in denied}}


def _action_admission_review(args: argparse.Namespace) -> dict[str, Any]:
    controller, coordinator = _action_admission_context(args.admission_store)
    row = controller.review(
        args.action_id,
        authorization_digest=args.authorization_digest,
        approvals=_action_review_approvals(args),
        reviewed=args.reviewed,
        reason=args.reason,
        expected_record_digest=args.expected_record_digest,
    )
    snapshot = coordinator.flush()
    return {
        "schema": CLI_SCHEMA,
        "command": "action-admission-review",
        "admission_store": args.admission_store,
        "row": row,
        "snapshot_digest": snapshot["snapshot_digest"],
        "queue": controller.queue(),
        "authorization": "external_authorization_digest_recorded;does_not_authorize_provider_source_tool_evaluator_credential_or_effect_dispatch",
        "retention": "metadata_only;task_prompt_and_runtime_values_not_returned",
        "secret_material": "never_returned",
    }


def _action_admission_handoff(args: argparse.Namespace) -> dict[str, Any]:
    controller, coordinator = _action_admission_context(args.admission_store)
    requested_domains = None if not args.domain else tuple(args.domain)
    handoff = controller.dispatch_handoff(args.action_id, requested_domains=requested_domains)
    return {
        "schema": CLI_SCHEMA,
        "command": "action-admission-handoff",
        "admission_store": args.admission_store,
        "handoff": handoff,
        "snapshot_digest": coordinator.expected_snapshot_digest,
        "authorization": "downstream_gate_handoff_only;provider_source_tool_evaluator_credential_and_effect_authority_remain_external",
        "retention": "metadata_only;task_prompt_and_runtime_values_not_returned",
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


def _activation_status(args: argparse.Namespace) -> dict[str, Any]:
    """Inspect a persisted activation snapshot without contacting providers or MCP."""

    store = AutonomousCapabilityActivationStore(args.activation_store)
    state = store.load()
    return {
        "schema": CLI_SCHEMA,
        "command": "activation-status",
        "available": state is not None,
        "activation_store": args.activation_store,
        "state": None if state is None else state.to_dict(),
        "authorization": "metadata_read_only; no_provider_or_credential_access",
        "retention": "redacted_activation_state_only; no_keys_handles_prompts_or_payloads",
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


def _load_evidence_file(path_value: str | None) -> dict[str, Any] | None:
    """Load bounded caller/evaluator evidence without treating it as provider output."""

    if path_value is None:
        return None
    path = Path(path_value)
    if not path.exists() or not path.is_file() or path.stat().st_size > _MAX_EVIDENCE_FILE_BYTES:
        raise ValueError("evidence file is missing or outside its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("evidence file is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping):
        raise ValueError("evidence file must contain a JSON object")
    return dict(raw)


def _load_grounded_portfolio_bundle(
    path_value: str | None,
    *,
    expected_schema: str,
) -> dict[str, Any] | None:
    """Load one caller-owned, real-data snapshot for the grounded portfolio command.

    The CLI only transports the snapshot to the authoritative MCP tools.  It still applies a
    small process-boundary contract first: bounded UTF-8 JSON, the expected source schema, an
    explicit non-synthetic marker, and no credential-shaped fields.
    """

    if path_value is None:
        return None
    path = _resolve_grounded_path(path_value)
    if not path.exists() or not path.is_file():
        raise ValueError("grounded portfolio bundle is missing")
    if path.stat().st_size > _MAX_GROUNDED_PORTFOLIO_BUNDLE_BYTES:
        raise ValueError("grounded portfolio bundle exceeds its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("grounded portfolio bundle is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping):
        raise ValueError("grounded portfolio bundle must contain a JSON object")
    if raw.get("schema_version") != expected_schema:
        raise ValueError("grounded portfolio bundle schema is invalid")
    if raw.get("synthetic_data") is not False:
        raise ValueError("grounded portfolio requires an explicit non-synthetic snapshot")
    return dict(raw)


def _load_grounded_case_asset_manifest(path_value: str | None) -> dict[str, Any] | None:
    """Load a caller-owned de-identified case manifest without opening any asset bytes."""

    if path_value is None:
        return None
    path = _resolve_grounded_path(path_value)
    if not path.exists() or not path.is_file():
        raise ValueError("grounded case-asset manifest is missing")
    if path.stat().st_size > _MAX_GROUNDED_PORTFOLIO_BUNDLE_BYTES:
        raise ValueError("grounded case-asset manifest exceeds its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("grounded case-asset manifest is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping):
        raise ValueError("grounded case-asset manifest must contain a JSON object")
    if raw.get("schema_version") != "bioprism-neurosurgery-case-asset-manifest/0.1":
        raise ValueError("grounded case-asset manifest schema is invalid")
    if raw.get("synthetic_data") is not False:
        raise ValueError("grounded case-asset manifest requires synthetic_data=false")
    if raw.get("direct_identifier_fields") not in (None, []):
        raise ValueError("grounded case-asset manifest contains direct identifier fields")
    return dict(raw)


def _resolve_grounded_path(path_value: str | os.PathLike[str]) -> Path:
    """Resolve a grounded snapshot path consistently for reads and atomic refreshes.

    The checked-in defaults are intentionally usable from either the repository root or the
    ``python`` package directory.  A caller-owned path that does not exist is left relative to
    the current working directory so an explicitly requested refresh can create it there.
    """

    path = Path(path_value)
    if not path.is_absolute() and not path.exists():
        repository_path = Path(__file__).resolve().parents[2] / path
        if repository_path.exists():
            path = repository_path
    return path


def _refresh_grounded_sources(
    *,
    real_data_path: str | None,
    public_literature_path: str | None,
    refresh_real_data: bool,
    refresh_public_literature: bool,
    approve_network: bool,
    timeout: float,
    resume: bool,
) -> dict[str, dict[str, Any]]:
    """Optionally refresh public snapshots before a grounded worker starts.

    Refresh is deliberately opt-in and cannot be combined with resume: a new snapshot changes
    the evidence digest, so silently mixing it into a persisted loop would make the checkpoint
    non-replayable.  Both refreshers are credentialless and atomically validate their candidate
    before replacing the caller-selected file.
    """

    requested = refresh_real_data or refresh_public_literature
    if not requested:
        return {}
    if resume:
        raise ValueError("source refresh cannot be combined with --resume; refresh first, then resume with the new digest")
    if not approve_network:
        raise ValueError("source refresh requires --approve-network")
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not 1 <= timeout <= 120:
        raise ValueError("--refresh-timeout must be between 1 and 120 seconds")
    if refresh_real_data and real_data_path is None:
        raise ValueError("--refresh-real-data requires the real-glioma plane")
    if refresh_public_literature and public_literature_path is None:
        raise ValueError("--refresh-public-literature requires the public-literature plane")

    reports: dict[str, dict[str, Any]] = {}
    if refresh_real_data:
        report = atomic_refresh_real_glioma_data(
            _resolve_grounded_path(real_data_path),
            timeout=float(timeout),
        )
        reports["real_glioma_population"] = (
            report.to_dict() if hasattr(report, "to_dict") else dict(report)
        )
    if refresh_public_literature:
        report = atomic_refresh_neurosurgical_public_literature(
            _resolve_grounded_path(public_literature_path),
            timeout=float(timeout),
        )
        reports["public_literature"] = (
            report.to_dict() if hasattr(report, "to_dict") else dict(report)
        )
    return reports


def _validate_grounded_source_refresh(value: Mapping[str, Any] | None) -> dict[str, Any]:
    """Validate the redacted refresh receipt retained by a grounded checkpoint.

    The receipt is intentionally separate from the source-plane loop digest: it records which
    credentialless public refreshes produced the snapshots used by the run, while the loops bind
    the resulting bundle digests.  Keeping this envelope in the checkpoint makes the provenance
    visible after restart without retaining request bodies, credentials, or patient data.
    """

    if value is None:
        value = {}
    if not isinstance(value, Mapping):
        raise ValueError("grounded source refresh receipt is invalid")
    _batch_request_json_safe(value)
    performed = value.get("performed", {})
    if not isinstance(performed, Mapping):
        raise ValueError("grounded source refresh receipt entries are invalid")
    allowed_planes = {"real_glioma_population", "public_literature"}
    if any(not isinstance(key, str) or key not in allowed_planes for key in performed):
        raise ValueError("grounded source refresh receipt names an unknown plane")
    if any(not isinstance(report, Mapping) for report in performed.values()):
        raise ValueError("grounded source refresh receipt report is invalid")
    expected = {
        "performed": dict(performed),
        "network_approved": bool(performed),
        "credentials_required": False,
        "synthetic_data": False,
        "human_review_required": True,
    }
    for key, expected_value in expected.items():
        if key in value and value.get(key) != expected_value:
            raise ValueError("grounded source refresh receipt safety posture is invalid")
    return expected


def _load_grounded_real_data_query(path_value: str | None) -> dict[str, Any] | None:
    """Load a bounded structured real-data query for the grounded operator commands."""

    if path_value is None:
        return None
    path = Path(path_value)
    if not path.is_absolute() and not path.exists():
        repository_path = Path(__file__).resolve().parents[2] / path
        if repository_path.exists():
            path = repository_path
    if not path.exists() or not path.is_file() or path.stat().st_size > _MAX_GROUNDED_QUERY_FILE_BYTES:
        raise ValueError("grounded real-data query file is missing or outside its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("grounded real-data query file is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping):
        raise ValueError("grounded real-data query file must contain a JSON object")
    return dict(raw)


def _load_grounded_public_literature_query(path_value: str | None) -> dict[str, Any] | None:
    """Load a bounded structured PubMed query for the grounded operator commands."""

    return _load_grounded_real_data_query(path_value)


def _grounded_portfolio_digest_descriptor(
    portfolio: Mapping[str, Any],
) -> dict[str, Any]:
    """Project exactly the digest descriptor emitted by ``LocalNeurosurgicalAgent``."""

    descriptor = {
        "schema_version": portfolio.get("schema_version"),
        "question_digest": portfolio.get("question_digest"),
        "provider": portfolio.get("provider"),
        "model": portfolio.get("model"),
        "specialty": portfolio.get("specialty"),
        "source_planes": portfolio.get("source_planes"),
        "real_data_bundle_digest": portfolio.get("real_data_bundle_digest"),
        "public_literature_bundle_digest": portfolio.get("public_literature_bundle_digest"),
        "real_data_loop_digest": (
            portfolio.get("real_data_loop", {}).get("loop_digest")
            if isinstance(portfolio.get("real_data_loop"), Mapping)
            else None
        ),
        "public_literature_loop_digest": (
            portfolio.get("public_literature_loop", {}).get("loop_digest")
            if isinstance(portfolio.get("public_literature_loop"), Mapping)
            else None
        ),
        "pending_real_data_queries": portfolio.get("pending_real_data_queries"),
        "pending_public_literature_queries": portfolio.get("pending_public_literature_queries"),
        "completed_pass_count": portfolio.get("completed_pass_count"),
        "claim_count": portfolio.get("claim_count"),
        "grounded_claim_count": portfolio.get("grounded_claim_count"),
        "blocked_claim_count": portfolio.get("blocked_claim_count"),
    }
    if "real_data_query" in portfolio:
        descriptor["real_data_query"] = portfolio.get("real_data_query")
    if "public_literature_query" in portfolio:
        descriptor["public_literature_query"] = portfolio.get("public_literature_query")
    # Preserve digest compatibility with pre-link-audit checkpoints while binding the new
    # optional reviewer artifact whenever it is present in a freshly emitted portfolio.
    if "literature_link_audit" in portfolio:
        link_audit = portfolio.get("literature_link_audit")
        descriptor["literature_link_audit_digest"] = (
            link_audit.get("audit_digest") if isinstance(link_audit, Mapping) else None
        )
    if "case_asset_manifest" in portfolio:
        manifest = portfolio.get("case_asset_manifest")
        descriptor["case_asset_manifest_digest"] = (
            manifest.get("report_digest") if isinstance(manifest, Mapping) else None
        )
    if "case_asset_manifest_query" in portfolio:
        descriptor["case_asset_manifest_query"] = portfolio.get("case_asset_manifest_query")
    return descriptor


def _validate_grounded_portfolio_result(value: Mapping[str, Any]) -> dict[str, Any]:
    """Verify the SDK portfolio envelope before it is persisted or resumed."""

    required = {
        "schema_version",
        "portfolio_digest",
        "question_digest",
        "provider",
        "model",
        "specialty",
        "source_planes",
        "real_data_loop",
        "public_literature_loop",
        "pending_real_data_queries",
        "pending_public_literature_queries",
    }
    if not isinstance(value, Mapping) or not required.issubset(value):
        raise ValueError("grounded portfolio result is incomplete")
    if value.get("schema_version") != "bioprism-neurosurgery-grounded-research-portfolio/0.1":
        raise ValueError("grounded portfolio result schema is invalid")
    digest = value.get("portfolio_digest")
    if not isinstance(digest, str) or digest != content_digest(_grounded_portfolio_digest_descriptor(value)):
        raise ValueError("grounded portfolio digest does not match its contents")
    planes = value.get("source_planes")
    if not isinstance(planes, list) or not planes or any(
        plane not in {"real_glioma_population", "public_literature"} for plane in planes
    ):
        raise ValueError("grounded portfolio source planes are invalid")
    real_loop = value.get("real_data_loop")
    public_loop = value.get("public_literature_loop")
    if "real_glioma_population" in planes and not isinstance(real_loop, Mapping):
        raise ValueError("grounded portfolio is missing its real-data loop")
    if "public_literature" in planes and not isinstance(public_loop, Mapping):
        raise ValueError("grounded portfolio is missing its literature loop")
    if "real_data_query" in value and not isinstance(value.get("real_data_query"), Mapping):
        raise ValueError("grounded portfolio real-data query is invalid")
    if "public_literature_query" in value and not isinstance(value.get("public_literature_query"), Mapping):
        raise ValueError("grounded portfolio public-literature query is invalid")
    return dict(value)


def _grounded_intake_digest_descriptor(value: Mapping[str, Any]) -> dict[str, Any]:
    """Project the exact envelope identity used by ``grounded_research_intake``."""

    portfolio = value.get("portfolio")
    return {
        "schema_version": value.get("schema_version"),
        "question_digest": value.get("question_digest"),
        "intake_digest": value.get("intake_digest"),
        "routed_specialty": value.get("routed_specialty"),
        "source_planes": value.get("source_planes"),
        "status": value.get("status"),
        "portfolio_digest": portfolio.get("portfolio_digest") if isinstance(portfolio, Mapping) else None,
    }


def _validate_grounded_intake_result(value: Mapping[str, Any]) -> dict[str, Any]:
    """Verify a persisted routed intake before allowing a worker to continue its loops."""

    required = {
        "schema_version",
        "envelope_digest",
        "question_digest",
        "intake_digest",
        "intake",
        "routed_specialty",
        "source_planes",
        "status",
        "portfolio",
        "required_evidence",
        "next_actions",
        "human_review_required",
    }
    if not isinstance(value, Mapping) or not required.issubset(value):
        raise ValueError("grounded intake result is incomplete")
    if value.get("schema_version") != "bioprism-neurosurgery-grounded-research-intake/0.1":
        raise ValueError("grounded intake result schema is invalid")
    digest = value.get("envelope_digest")
    if not isinstance(digest, str) or digest != content_digest(_grounded_intake_digest_descriptor(value)):
        raise ValueError("grounded intake envelope digest does not match its contents")
    question_digest = value.get("question_digest")
    intake_digest = value.get("intake_digest")
    if (
        not isinstance(question_digest, str)
        or len(question_digest) != 64
        or any(character not in "0123456789abcdef" for character in question_digest)
        or not isinstance(intake_digest, str)
        or len(intake_digest) != 64
        or any(character not in "0123456789abcdef" for character in intake_digest)
    ):
        raise ValueError("grounded intake digest fields are invalid")
    routed = value.get("routed_specialty")
    supported = {
        "glioma",
        "cranial_base",
        "craniosynostosis",
        "encephalocele",
        "spina_bifida",
        "chiari_malformation",
    }
    if routed is not None and routed not in supported:
        raise ValueError("grounded intake specialty is invalid")
    intake = value.get("intake")
    if (
        not isinstance(intake, Mapping)
        or intake.get("plan_digest") != intake_digest
        or intake.get("question_digest") != question_digest
        or intake.get("selected_specialty") != routed
    ):
        raise ValueError("grounded intake plan digest is inconsistent")
    planes = value.get("source_planes")
    if not isinstance(planes, list) or any(plane not in {"real_glioma_population", "public_literature"} for plane in planes):
        raise ValueError("grounded intake source planes are invalid")
    status = value.get("status")
    if status not in {"abstained", "needs_evidence", "grounded_for_human_review", "blocked"}:
        raise ValueError("grounded intake status is invalid")
    portfolio = value.get("portfolio")
    if portfolio is None:
        if status not in {"abstained", "needs_evidence"} or planes:
            raise ValueError("grounded intake hold is inconsistent")
    else:
        if not isinstance(portfolio, Mapping):
            raise ValueError("grounded intake portfolio must be an object")
        verified_portfolio = _validate_grounded_portfolio_result(portfolio)
        if status not in {"grounded_for_human_review", "blocked"}:
            raise ValueError("grounded intake portfolio status is inconsistent")
        if verified_portfolio.get("question_digest") != question_digest or verified_portfolio.get("source_planes") != planes:
            raise ValueError("grounded intake portfolio identity is inconsistent")
        value = {**dict(value), "portfolio": verified_portfolio}
    if value.get("human_review_required") is not True:
        raise ValueError("grounded intake must remain human-review gated")
    for field in ("required_evidence", "next_actions"):
        if not isinstance(value.get(field), list):
            raise ValueError(f"grounded intake {field} must be a list")
    return dict(value)


def _load_grounded_intake_store(path_value: str) -> dict[str, Any]:
    """Read and verify a digest-bound routed-intake checkpoint."""

    path = Path(path_value)
    if not path.exists() or not path.is_file():
        raise ValueError("grounded intake resume requires an existing output store")
    if path.stat().st_size > _MAX_GROUNDED_PORTFOLIO_STORE_BYTES:
        raise ValueError("grounded intake output store exceeds its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("grounded intake output store is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping) or raw.get("schema") != GROUNDED_INTAKE_STORE_SCHEMA:
        raise ValueError("grounded intake output store has an invalid schema")
    unsigned = dict(raw)
    supplied_digest = unsigned.pop("store_digest", None)
    if not isinstance(supplied_digest, str) or supplied_digest != content_digest(unsigned):
        raise ValueError("grounded intake output store digest does not match its contents")
    if raw.get("command") != "grounded-autopilot":
        raise ValueError("grounded intake output command marker is invalid")
    intake = raw.get("intake")
    if not isinstance(intake, Mapping):
        raise ValueError("grounded intake output is missing its intake")
    verified = _validate_grounded_intake_result(intake)
    raw_provider = raw.get("provider")
    if raw_provider not in {"ollama", "local", "in_memory"}:
        raise ValueError("grounded intake output provider is invalid")
    raw_model = raw.get("model")
    if not isinstance(raw_model, str) or not raw_model.strip():
        raise ValueError("grounded intake output model is invalid")
    portfolio = verified.get("portfolio")
    if isinstance(portfolio, Mapping):
        if raw_provider != portfolio.get("provider"):
            raise ValueError("grounded intake output provider does not match its portfolio")
        if raw_model != portfolio.get("model"):
            raise ValueError("grounded intake output model does not match its portfolio")
    if raw.get("question_digest") != verified.get("question_digest"):
        raise ValueError("grounded intake output question identity does not match its intake")
    if raw.get("routed_specialty") != verified.get("routed_specialty"):
        raise ValueError("grounded intake output specialty identity does not match its intake")
    source_paths = raw.get("source_paths")
    if not isinstance(source_paths, Mapping):
        raise ValueError("grounded intake output source paths are invalid")
    controls = raw.get("controls")
    if not isinstance(controls, Mapping):
        raise ValueError("grounded intake output controls are invalid")
    source_refresh = _validate_grounded_source_refresh(raw.get("source_refresh"))
    return {
        **dict(raw),
        "intake": verified,
        "source_paths": dict(source_paths),
        "controls": dict(controls),
        "source_refresh": source_refresh,
    }


def _persist_grounded_intake_store(
    path_value: str,
    *,
    intake: Mapping[str, Any],
    provider: str,
    model: str,
    source_paths: Mapping[str, str | None],
    controls: Mapping[str, Any],
    source_refresh: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Atomically persist the complete routed intake and any caller-owned portfolio ledger."""

    verified = _validate_grounded_intake_result(intake)
    unsigned: dict[str, Any] = {
        "schema": GROUNDED_INTAKE_STORE_SCHEMA,
        "command": "grounded-autopilot",
        "provider": provider,
        "model": model,
        "question_digest": verified["question_digest"],
        "routed_specialty": verified["routed_specialty"],
        "source_paths": dict(source_paths),
        "controls": dict(controls),
        "source_refresh": _validate_grounded_source_refresh(source_refresh),
        "intake": verified,
        "retention": "caller_owned_grounded_answers_and_claims; no_credentials_or_patient_data",
    }
    payload = {**unsigned, "store_digest": content_digest(unsigned)}
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
    if len(encoded.encode("utf-8")) > _MAX_GROUNDED_PORTFOLIO_STORE_BYTES:
        raise ValueError("grounded intake output store exceeds its bounded size")
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
    persisted = _load_grounded_intake_store(str(destination))
    return persisted


def _load_grounded_portfolio_store(path_value: str) -> dict[str, Any]:
    """Read and verify a digest-bound grounded portfolio checkpoint."""

    path = Path(path_value)
    if not path.exists() or not path.is_file():
        raise ValueError("grounded portfolio resume requires an existing output store")
    if path.stat().st_size > _MAX_GROUNDED_PORTFOLIO_STORE_BYTES:
        raise ValueError("grounded portfolio output store exceeds its bounded size")
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("grounded portfolio output store is unreadable") from error
    _batch_request_json_safe(raw)
    if not isinstance(raw, Mapping) or raw.get("schema") != GROUNDED_PORTFOLIO_CLI_SCHEMA:
        raise ValueError("grounded portfolio output store has an invalid schema")
    supplied_digest = raw.get("store_digest")
    unsigned = dict(raw)
    unsigned.pop("store_digest", None)
    if supplied_digest != content_digest(unsigned):
        raise ValueError("grounded portfolio output store digest does not match its contents")
    if raw.get("command") != "grounded-portfolio":
        raise ValueError("grounded portfolio output command marker is invalid")
    portfolio = raw.get("portfolio")
    if not isinstance(portfolio, Mapping):
        raise ValueError("grounded portfolio output is missing its portfolio")
    if (
        raw.get("provider") != portfolio.get("provider")
        or raw.get("model") != portfolio.get("model")
        or raw.get("question_digest") != portfolio.get("question_digest")
    ):
        raise ValueError("grounded portfolio output identity does not match its portfolio")
    source_refresh = _validate_grounded_source_refresh(raw.get("source_refresh"))
    return {
        **dict(raw),
        "portfolio": _validate_grounded_portfolio_result(portfolio),
        "source_refresh": source_refresh,
    }


def _persist_grounded_portfolio_store(
    path_value: str,
    *,
    portfolio: Mapping[str, Any],
    provider: str,
    model: str,
    source_paths: Mapping[str, str | None],
    source_refresh: Mapping[str, Any] | None = None,
) -> None:
    """Atomically persist a complete, digest-bound loop ledger for restart/resume."""

    verified = _validate_grounded_portfolio_result(portfolio)
    unsigned: dict[str, Any] = {
        "schema": GROUNDED_PORTFOLIO_CLI_SCHEMA,
        "command": "grounded-portfolio",
        "provider": provider,
        "model": model,
        "question_digest": verified["question_digest"],
        "source_paths": dict(source_paths),
        "source_refresh": _validate_grounded_source_refresh(source_refresh),
        "portfolio": verified,
        "retention": "caller_owned_grounded_answers_and_claims; no_credentials_or_patient_data",
    }
    payload = {**unsigned, "store_digest": content_digest(unsigned)}
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False) + "\n"
    if len(encoded.encode("utf-8")) > _MAX_GROUNDED_PORTFOLIO_STORE_BYTES:
        raise ValueError("grounded portfolio output store exceeds its bounded size")
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


def _load_launch_admission_file(path_value: str | None) -> dict[str, Any] | None:
    """Load one bounded, digest-verified admission record without exposing its contents."""

    if path_value is None:
        return None
    path = Path(path_value)
    try:
        if not path.exists() or not path.is_file():
            raise ValueError("launch admission file is missing")
        if path.stat().st_size > MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES:
            raise ValueError("launch admission file is outside its bounded size")
        encoded = path.read_bytes()
        if len(encoded) > MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES:
            raise ValueError("launch admission file is outside its bounded size")
        value = json.loads(encoded.decode("utf-8"))
        if not isinstance(value, Mapping):
            raise ValueError("launch admission file must contain a JSON object")
        return validate_autonomous_launch_admission(value)
    except ValueError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ValueError("launch admission file is unreadable or invalid") from error


def _launch_admission_projection(
    admission: Mapping[str, Any] | None,
) -> dict[str, Any]:
    """Project admission status while excluding its reason and all transient values."""

    if admission is None:
        return {
            "configured": False,
            "status": None,
            "admission_id": None,
            "admission_digest": None,
            "approved_domains": [],
            "retention": "metadata_only_admission_identity_and_scope",
        }
    approved_domains = sorted(
        row["domain"]
        for row in admission["domains"]
        if row.get("admission_state") == "approved"
    )
    return {
        "configured": True,
        "status": admission["status"],
        "decision": admission["decision"],
        "admission_id": admission["admission_id"],
        "admission_digest": admission["admission_digest"],
        "preflight_report_digest": admission["preflight_report_digest"],
        "approved_domains": approved_domains,
        "summary": dict(admission["summary"]),
        "retention": "metadata_only_admission_identity_and_scope",
        "secret_material": "never_returned",
    }


def _route_options_for_admission(options: Mapping[str, Any]) -> dict[str, Any]:
    """Keep automatic admission previews aligned with the provider-free router contract."""

    return {
        key: options[key]
        for key in (
            "hints",
            "min_confidence",
            "min_margin",
            "max_domains",
            "allow_cross_domain",
            "context",
            "constraints",
            "desired_outputs",
            "capability",
            "risk_class",
            "max_steps",
            "require_json",
            "structured_domain_response",
            "response_schema",
            "execution_mode",
            "max_input_tokens",
            "required_model_capabilities",
            "memory_episodes",
        )
        if key in options
    }


def _preflight_cli_launch_admission(
    admission: Mapping[str, Any] | None,
    runtime: LLMRuntime,
    *,
    task: str | None = None,
    domain: str | None = None,
    automatic: bool = False,
    hints: Sequence[str] = (),
    max_domains: int = 3,
    allow_cross_domain: bool = True,
    semantic_routing: bool = False,
    requests: Sequence[Mapping[str, Any]] = (),
    mode: str | None = None,
    single_domain: bool = False,
    workflow_execution: bool = False,
) -> None:
    """Reject an under-scoped CLI launch before credential collection or MCP startup."""

    if admission is None:
        return
    offline_agent = AutonomousAgent(
        _OfflineWorkspace(),
        runtime,
        model_catalogue=ModelCatalogue(),
    )
    if automatic:
        if task is None:
            raise ValueError("automatic launch admission requires a task")
        offline_agent.authorize_auto_launch_admission(
            task=task,
            launch_admission=admission,
            hints=tuple(hints),
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            semantic_routing=semantic_routing,
        )
        return
    if domain is not None:
        authorize_autonomous_launch_domains(admission, (domain,))
        return
    if mode is None:
        raise ValueError("launch admission scope is missing a mode")
    requested_domains: list[str] = []
    for index, request in enumerate(requests):
        if mode == "domain":
            requested = request.get("domain")
            if not isinstance(requested, str):
                raise ValueError(f"batch request {index} is missing a domain")
            requested_domains.append(requested)
        elif mode == "cross_domain":
            subtasks = request.get("subtasks", ())
            for subtask in subtasks:
                if isinstance(subtask, Mapping) and isinstance(subtask.get("domain"), str):
                    requested_domains.append(subtask["domain"])
        elif mode == "auto":
            options = dict(request.get("options", {}))
            if single_domain or workflow_execution:
                options["allow_cross_domain"] = False
            offline_agent.authorize_auto_launch_admission(
                task=request["task"],
                launch_admission=admission,
                semantic_routing=options.get("semantic_routing", False),
                **_route_options_for_admission(options),
            )
        else:
            raise ValueError("launch admission mode is unsupported")
    if mode != "auto":
        authorize_autonomous_launch_domains(admission, tuple(dict.fromkeys(requested_domains)))


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
        if mode != "domain" and "capability" in options:
            raise ValueError("batch capability options require domain mode")
        normalized = dict(request)
        normalized["options"] = dict(options)
        normalized_requests.append(normalized)
    return mode, job_id, normalized_requests


def _open_cli_memory(
    args: argparse.Namespace,
    *,
    learning_requested: bool,
) -> BrainEpisodicMemory | None:
    """Open caller-owned metadata memory for recall and evaluator-gated learning.

    Learning requires an episodic store even when the caller only wants value-only bandit
    persistence.  A process-local SQLite database keeps that requirement explicit for ordinary
    runs, while ``--memory-store`` makes the same digest-only memory restart-safe.  The learning
    ledger and episodic memory must remain separate SQLite files so their schemas cannot collide.
    """

    path = getattr(args, "memory_store", None)
    learning_store = getattr(args, "learning_store", None)
    if path is not None and learning_store is not None:
        try:
            same_path = Path(path).resolve() == Path(learning_store).resolve()
        except OSError as error:
            raise ValueError("memory and learning store paths are invalid") from error
        if same_path:
            raise ValueError("--memory-store must be different from --learning-store")
    if path is None and not learning_requested:
        return None
    return BrainEpisodicMemory(":memory:" if path is None else path)


def _cli_learning_state(
    agent: Any,
    *,
    domain: str,
    capability: str | None,
) -> Mapping[str, Any]:
    """Resolve a contextual first-run bandit projection without exposing learned values."""

    contextual = getattr(agent, "domain_learning_state", None)
    if callable(contextual):
        snapshot = contextual(domain, capability=capability)
        if isinstance(snapshot, Mapping) and isinstance(snapshot.get("bandit_state"), Mapping):
            return dict(snapshot["bandit_state"])
    generic = getattr(agent, "learning_state", None)
    if callable(generic):
        state = generic()
        if isinstance(state, Mapping):
            return dict(state)
    return {
        "schema": "bioprism-brain-bandit/0.1",
        "generation": 0,
        "arms": [],
    }


def _batch_run(
    args: argparse.Namespace,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
    client_factory: Callable[..., Client] = Client,
) -> dict[str, Any]:
    mode, job_id, requests = _load_batch_requests(args)
    if args.domain_tool_domain and not (args.activate_domain_tools or args.approve_domain_tool):
        raise ValueError("--domain-tool-domain requires --activate-domain-tools or --approve-domain-tool")
    if args.domain_tool_bindings_file and args.domain_tool_domain:
        raise ValueError("--domain-tool-domain cannot be combined with --domain-tool-bindings-file")
    if args.domain_tool_bindings_file and (
        args.activate_domain_tools or args.approve_domain_tool or args.resume_activation
    ):
        raise ValueError("--domain-tool-bindings-file cannot be combined with curated activation flags")
    activation_store, loaded_activation_state, activation_resumed = _load_activation_store(args)
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
    evidence = _load_evidence_file(args.evidence_file)
    if evidence is not None and mode == "cross_domain":
        raise ValueError("--evidence-file requires domain or automatic batch mode; use per-request evidence for cross-domain work")
    launch_admission = _load_launch_admission_file(args.launch_admission_file)
    request_learning = any(
        isinstance(request.get("options"), Mapping)
        and request.get("options", {}).get("learn") is True
        for request in requests
    )
    persisted_candidates = _persisted_candidate_args(args) if args.use_inventory else ()
    runtime, onboarding = _runtime_with_provider(args)
    _preflight_cli_launch_admission(
        launch_admission,
        runtime,
        requests=requests,
        mode=mode,
        single_domain=args.single_domain,
        workflow_execution=args.workflow_execution,
    )
    session = onboarding.start_session(ttl_seconds=args.ttl_seconds)
    health_ledger = None
    learning_ledger = None
    learning_memory = None
    execution_journal = None
    activation_state_after: Mapping[str, Any] | None = None
    activation_persisted = False
    tool_surface: dict[str, Any] = {
        "mode": "not_requested",
        "exposed_count": 0,
        "catalogue_digest": None,
        "exposed_tools": [],
        "domain_binding": {"requested": False},
        "authority": "not_applicable",
        "retention": "tool_names_and_schema_digest_only",
    }
    try:
        if args.health_store is not None:
            health_ledger = ProviderHealthLedger(args.health_store)
        if args.learning_store is not None:
            learning_ledger = SQLiteBrainLearningLedger(args.learning_store)
        learning_memory = _open_cli_memory(
            args,
            learning_requested=(args.learning_mode in {"online", "trajectory"} or request_learning),
        )
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
            if loaded_activation_state is not None:
                agent_kwargs["activation"] = AutonomousCapabilityActivation(
                    state=loaded_activation_state
                )
            agent = AutonomousAgent(_McpWorkspace(client), runtime, **agent_kwargs)
            needs_provider_tools = args.execution_mode in {"tool_loop", "mission"} or any(
                isinstance(request.get("options"), Mapping)
                and request["options"].get("execution_mode") in {"tool_loop", "mission"}
                for request in requests
            )
            domain_activation_requested = bool(
                args.activate_domain_tools or args.approve_domain_tool
            )
            binding_file_requested = args.domain_tool_bindings_file is not None
            domain_registry_requested = (
                domain_activation_requested or activation_resumed or binding_file_requested
            )
            provider_tools, tool_catalogue = (
                _mcp_provider_tools(
                    client,
                    allow_tools=tuple(args.allow_mcp_tool or ()),
                    deny_tools=tuple(args.deny_mcp_tool or ()),
                )
                if needs_provider_tools or domain_registry_requested
                else ((), None)
            )
            domain_binding = None
            if binding_file_requested:
                assert tool_catalogue is not None
                domain_binding = _register_domain_tool_bindings_file(
                    agent,
                    tool_catalogue,
                    args.domain_tool_bindings_file,
                    approve_mission_dispatch=args.approve_mission_dispatch,
                )
                tool_surface["domain_binding"] = domain_binding
                tool_surface["mode"] = "domain_registry"
            elif domain_activation_requested:
                assert tool_catalogue is not None
                domain_binding = _activate_domain_tools(
                    agent,
                    tool_catalogue,
                    domains=_domain_tool_domains(args, batch=True),
                    activate=args.activate_domain_tools,
                    approved_tools=tuple(args.approve_domain_tool or ()),
                )
                assert domain_binding is not None
            elif activation_resumed:
                assert tool_catalogue is not None
                domain_binding = _rehydrate_domain_tools(
                    agent,
                    tool_catalogue,
                    previous_state=loaded_activation_state.to_dict(),
                )
            tool_authorizer = (
                _cli_tool_authorizer(
                    client,
                    catalogue=tool_catalogue,
                    approve_mission_dispatch=args.approve_mission_dispatch,
                    allowed_tools=(
                        tuple(domain_binding.get("registered_tools", ()))
                        if domain_binding is not None
                        else None
                    ),
                    read_only_tools=_registered_tool_posture(agent),
                )
                if provider_tools and tool_catalogue is not None
                else None
            )
            if needs_provider_tools or domain_registry_requested:
                assert tool_catalogue is not None
                tool_surface = {
                    "mode": "live_mcp",
                    "exposed_count": len(tool_catalogue.definitions),
                    "catalogue_digest": tool_catalogue.digest,
                    "exposed_tools": sorted(
                        definition.name for definition in tool_catalogue.definitions
                    ),
                    "allowlist": sorted(set(args.allow_mcp_tool or ())),
                    "denylist": sorted(set(args.deny_mcp_tool or ())),
                    "authority": "caller_approved_only",
                    "retention": "tool_names_and_schema_digest_only",
                }
                if domain_binding is not None:
                    tool_surface["domain_binding"] = domain_binding
                    tool_surface["mode"] = "domain_registry"
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
                if learning_memory is not None:
                    options["memory"] = learning_memory
                if evidence is not None:
                    options["evidence"] = evidence
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
                options.pop("approve_capability", None)
                if options.get("capability") is not None:
                    options["approve_capability"] = args.approve_capability
                if mode == "domain" and options.get("learn") is True:
                    options["bandit_state"] = _cli_learning_state(
                        agent,
                        domain=raw["domain"],
                        capability=options.get("capability"),
                    )
                if args.workflow_execution:
                    options["workflow_execution"] = True
                    options["allow_cross_domain"] = False
                    if args.workflow_max_stage_calls is not None:
                        options["workflow_max_stage_calls"] = args.workflow_max_stage_calls
                    if args.workflow_retry_blocked:
                        options["workflow_retry_blocked"] = True
                if (
                    provider_tools
                    and not domain_registry_requested
                    and options.get("execution_mode", "provider") in {"tool_loop", "mission"}
                ):
                    options["provider_tools"] = provider_tools
                    options["tool_loop_options"] = {
                        "authorize_and_execute": tool_authorizer,
                    }
                elif (
                    domain_registry_requested
                    and options.get("execution_mode", "provider") in {"tool_loop", "mission"}
                ):
                    options["tool_loop_options"] = {
                        "authorize_and_execute": tool_authorizer,
                    }
                return options

            run_payload = controller.run(
                requests,
                job_id=job_id,
                mode=mode,
                launch_admission=launch_admission,
                credentials=session,
                model_candidates=candidates,
                options_factory=options_factory,
                max_parallelism=args.max_parallelism,
                stop_on_error=args.stop_on_error,
                rehydrate_result=rehydrate_result,
            )
            if activation_store is not None:
                agent.save_activation(activation_store)
                activation_state_after = agent.activation_state()
                activation_persisted = True
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
            "launch_admission": _launch_admission_projection(launch_admission),
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
            "tool_surface": tool_surface,
            "activation_persistence": _activation_persistence_projection(
                activation_store,
                resumed=activation_resumed,
                persisted=activation_persisted,
                state=activation_state_after,
            ),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": args.approve_provider_call,
                "capability_approved": args.approve_capability,
                "mission_dispatch_approved": args.approve_mission_dispatch,
            },
            "learning": {
                "learning_mode": args.learning_mode,
                "learning_store_configured": learning_ledger is not None,
                "memory_store_configured": learning_memory is not None,
                "memory_retention": "digest_only_episodic_metadata",
            },
            "secret_material": "never_returned",
        }
    finally:
        session.close()
        if learning_memory is not None:
            learning_memory.close()
        if learning_ledger is not None:
            learning_ledger.close()


def _grounded_portfolio(
    args: argparse.Namespace,
    *,
    client_factory: Callable[..., Client] = Client,
) -> dict[str, Any]:
    """Run the source-separated neurosurgical portfolio from a no-key operator process."""

    if args.provider not in {"ollama", "local", "in_memory"}:
        raise ValueError("grounded-portfolio only permits credentialless local or loopback providers")
    if not args.approve_provider_call:
        raise ValueError("grounded-portfolio requires --approve-provider-call")
    if args.without_real_data and args.without_public_literature:
        raise ValueError("at least one grounded portfolio source must be enabled")
    if (args.freshness_as_of is None) != (args.freshness_max_age_days is None):
        raise ValueError("--freshness-as-of and --freshness-max-age-days must be supplied together")
    if not 1 <= args.max_passes <= 8:
        raise ValueError("--max-passes must be between 1 and 8")
    if not 0 <= args.max_follow_ups_per_pass <= 8:
        raise ValueError("--max-follow-ups-per-pass must be between 0 and 8")
    if not 128 <= args.max_output_tokens <= 16_384:
        raise ValueError("--max-output-tokens must be between 128 and 16384")
    if not 1 <= args.max_hits <= 128:
        raise ValueError("--max-hits must be between 1 and 128")
    if not 1 <= args.max_chars <= 65_536:
        raise ValueError("--max-chars must be between 1 and 65536")
    if not 1 <= args.max_tool_turns <= 8:
        raise ValueError("--max-tool-turns must be between 1 and 8")
    if not 1 <= args.max_tool_calls <= 32:
        raise ValueError("--max-tool-calls must be between 1 and 32")

    real_data_path = None if args.without_real_data else args.real_data_file
    public_literature_path = (
        None if args.without_public_literature else args.public_literature_file
    )
    source_refresh = _refresh_grounded_sources(
        real_data_path=real_data_path,
        public_literature_path=public_literature_path,
        refresh_real_data=args.refresh_real_data,
        refresh_public_literature=args.refresh_public_literature,
        approve_network=args.approve_network,
        timeout=args.refresh_timeout,
        resume=args.resume,
    )
    source_refresh_payload = _validate_grounded_source_refresh({"performed": source_refresh})
    real_data = _load_grounded_portfolio_bundle(
        real_data_path,
        expected_schema="bioprism-neurosurgery-real/0.1",
    )
    public_literature = _load_grounded_portfolio_bundle(
        public_literature_path,
        expected_schema="bioprism-neurosurgery-public-literature/0.1",
    )
    real_data_query = _load_grounded_real_data_query(args.real_data_query_file)
    public_literature_query = _load_grounded_public_literature_query(args.public_literature_query_file)
    case_asset_manifest = _load_grounded_case_asset_manifest(args.case_asset_manifest)
    case_asset_manifest_query = _load_grounded_real_data_query(args.case_asset_manifest_query)
    if case_asset_manifest_query is not None and case_asset_manifest is None:
        raise ValueError("--case-asset-manifest-query requires --case-asset-manifest")
    if real_data_query is not None and real_data is None:
        raise ValueError("--real-data-query-file requires the real-glioma plane")
    if public_literature_query is not None and public_literature is None:
        raise ValueError("--public-literature-query-file requires the public-literature plane")
    source_paths: dict[str, str | None] = {
        "real_data_file": real_data_path,
        "public_literature_file": public_literature_path,
    }
    if args.real_data_query_file is not None:
        source_paths["real_data_query_file"] = args.real_data_query_file
    if args.public_literature_query_file is not None:
        source_paths["public_literature_query_file"] = args.public_literature_query_file
    if args.case_asset_manifest is not None:
        source_paths["case_asset_manifest"] = args.case_asset_manifest
        source_paths["case_asset_manifest_content_digest"] = content_digest(case_asset_manifest)
    if args.case_asset_manifest_query is not None:
        source_paths["case_asset_manifest_query"] = args.case_asset_manifest_query
        source_paths["case_asset_manifest_query_content_digest"] = content_digest(case_asset_manifest_query)
    output_path = Path(args.portfolio_output)
    prior_store = _load_grounded_portfolio_store(str(output_path)) if args.resume else None
    if prior_store is not None and not source_refresh:
        source_refresh_payload = prior_store["source_refresh"]
    if output_path.exists() and not args.resume:
        raise ValueError("existing grounded portfolio output requires --resume")
    prior_portfolio = None if prior_store is None else prior_store["portfolio"]
    expected_planes = [
        plane
        for plane, enabled in (
            ("real_glioma_population", real_data is not None),
            ("public_literature", public_literature is not None),
        )
        if enabled
    ]
    if prior_portfolio is not None:
        question_digest = hashlib.sha256(args.question.encode("utf-8")).hexdigest()
        if prior_portfolio.get("question_digest") != question_digest:
            raise ValueError("--resume question does not match the persisted portfolio")
        if prior_portfolio.get("provider") != args.provider or prior_portfolio.get("model") != args.model:
            raise ValueError("--resume provider/model does not match the persisted portfolio")
        if prior_portfolio.get("specialty") != args.specialty:
            raise ValueError("--resume specialty does not match the persisted portfolio")
        if prior_portfolio.get("source_planes") != expected_planes:
            raise ValueError("--resume source selection does not match the persisted portfolio")
        if dict(prior_store.get("source_paths", {})) != source_paths:
            raise ValueError("--resume source paths do not match the persisted portfolio")

    freshness = None
    if args.freshness_as_of is not None:
        freshness = {
            "as_of": args.freshness_as_of,
            "max_age_days": args.freshness_max_age_days,
        }
    command = _parse_mcp_command(args.mcp_command)
    runtime, onboarding = _runtime_with_provider(args)
    session = onboarding.start_session()
    try:
        client = client_factory(command, cwd=args.mcp_cwd, timeout=args.mcp_timeout)
        with client:
            agent = LocalNeurosurgicalAgent(client)
            portfolio = agent.grounded_research_portfolio(
                args.question,
                runtime,
                args.provider,
                args.model,
                real_glioma_data=real_data,
                public_literature=public_literature,
                case_asset_manifest=case_asset_manifest,
                case_asset_manifest_query=case_asset_manifest_query,
                specialty=args.specialty,
                approve_provider_call=True,
                max_passes=args.max_passes,
                max_follow_ups_per_pass=args.max_follow_ups_per_pass,
                max_output_tokens=args.max_output_tokens,
                max_hits=args.max_hits,
                max_chars=args.max_chars,
                include_abstracts=not args.no_abstracts,
                freshness=freshness,
                real_data_query=real_data_query,
                public_literature_query=public_literature_query,
                tool_loop=args.tool_loop,
                max_tool_turns=args.max_tool_turns,
                max_tool_calls=args.max_tool_calls,
                real_resume_from=(
                    None
                    if prior_portfolio is None
                    else prior_portfolio.get("real_data_loop")
                ),
                public_resume_from=(
                    None
                    if prior_portfolio is None
                    else prior_portfolio.get("public_literature_loop")
                ),
            )
        _persist_grounded_portfolio_store(
            str(output_path),
            portfolio=portfolio,
            provider=args.provider,
            model=args.model,
            source_paths=source_paths,
            source_refresh=source_refresh_payload,
        )
        persisted_store = _load_grounded_portfolio_store(str(output_path))
        return {
            "schema": CLI_SCHEMA,
            "command": "grounded-portfolio",
            "portfolio": portfolio,
            "portfolio_output": str(output_path),
            "source_refresh": source_refresh_payload,
            "persistence": {
                "store": str(output_path),
                "store_digest": persisted_store["store_digest"],
                "resume_requested": args.resume,
                "resumed": prior_portfolio is not None,
                "retention": "caller_owned_grounded_answers_and_claims; no_credentials_or_patient_data",
            },
            "provider_status": runtime.provider_status(args.provider),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": True,
                "network": args.provider == "ollama" or bool(source_refresh),
                "source_refresh_network_approved": bool(source_refresh),
                "clinical_actions_authorized": False,
                "human_review_required": True,
            },
            "secret_material": "never_returned",
        }
    finally:
        session.close()


def _refresh_public_literature(
    args: argparse.Namespace,
    *,
    refresher: Callable[..., Any] | None = None,
) -> dict[str, Any]:
    """Refresh the checked-in PubMed plane through the credentialless public-data boundary."""

    if not args.approve_network:
        raise ValueError("refresh-public-literature requires --approve-network")
    if not 1 <= args.per_specialty_limit <= 50:
        raise ValueError("--per-specialty-limit must be between 1 and 50")
    if not 1 <= args.timeout <= 120:
        raise ValueError("--timeout must be between 1 and 120 seconds")
    if refresher is None:
        refresher = atomic_refresh_neurosurgical_public_literature
    report = refresher(
        args.output,
        per_specialty_limit=args.per_specialty_limit,
        timeout=args.timeout,
    )
    projected = report.to_dict() if hasattr(report, "to_dict") else dict(report)
    return {
        "schema": CLI_SCHEMA,
        "refresh_schema": PUBLIC_LITERATURE_REFRESH_CLI_SCHEMA,
        "command": "refresh-public-literature",
        "refresh": projected,
        "authorization": {
            "network_approved": True,
            "credentials_required": False,
            "synthetic_data": False,
            "human_review_required": True,
        },
        "secret_material": "never_returned",
    }


def _refresh_real_glioma(args: argparse.Namespace) -> dict[str, Any]:
    """Refresh the public aggregate glioma bundle through the credentialless network edge."""

    if not args.approve_network:
        raise ValueError("refresh-real-glioma requires --approve-network")
    gdc_project_ids = tuple(args.gdc_project_id or DEFAULT_GDC_PROJECT_IDS)
    portal_study_ids = tuple(args.portal_study_id or DEFAULT_PORTAL_STUDY_IDS)
    report = atomic_refresh_real_glioma_data(
        args.output,
        gdc_project_ids=gdc_project_ids,
        trial_page_size=args.trial_page_size,
        portal_study_ids=portal_study_ids,
        portal_study_limit=args.portal_study_limit,
        pubmed_limit=args.pubmed_limit,
        pubmed_term=args.pubmed_term or DEFAULT_PUBMED_TERM,
        pubmed_source_id=args.pubmed_source_id or DEFAULT_PUBMED_SOURCE_ID,
        timeout=args.timeout,
    )
    projected = report.to_dict() if hasattr(report, "to_dict") else dict(report)
    return {
        "schema": CLI_SCHEMA,
        "refresh_schema": REAL_DATA_REFRESH_CLI_SCHEMA,
        "command": "refresh-real-glioma",
        "refresh": projected,
        "authorization": {
            "network_approved": True,
            "credentials_required": False,
            "synthetic_data": False,
            "patient_data_access": False,
            "human_review_required": True,
        },
        "secret_material": "never_returned",
    }


def _grounded_autopilot(
    args: argparse.Namespace,
    *,
    client_factory: Callable[..., Client] = Client,
) -> dict[str, Any]:
    """Route and run one source-gated neurosurgical research question without a key."""

    if args.provider not in {"ollama", "local", "in_memory"}:
        raise ValueError("grounded-autopilot only permits credentialless local or loopback providers")
    if not args.approve_provider_call:
        raise ValueError("grounded-autopilot requires --approve-provider-call")
    if (args.freshness_as_of is None) != (args.freshness_max_age_days is None):
        raise ValueError("--freshness-as-of and --freshness-max-age-days must be supplied together")
    if not 1 <= args.max_passes <= 8:
        raise ValueError("--max-passes must be between 1 and 8")
    if not 0 <= args.max_follow_ups_per_pass <= 8:
        raise ValueError("--max-follow-ups-per-pass must be between 0 and 8")
    if not 128 <= args.max_output_tokens <= 16_384:
        raise ValueError("--max-output-tokens must be between 128 and 16384")
    if not 1 <= args.max_hits <= 128:
        raise ValueError("--max-hits must be between 1 and 128")
    if not 1 <= args.max_chars <= 65_536:
        raise ValueError("--max-chars must be between 1 and 65536")
    if not 1 <= args.max_tool_turns <= 8:
        raise ValueError("--max-tool-turns must be between 1 and 8")
    if not 1 <= args.max_tool_calls <= 32:
        raise ValueError("--max-tool-calls must be between 1 and 32")

    output_path = Path(args.intake_output)
    prior_store = _load_grounded_intake_store(str(output_path)) if args.resume else None
    if output_path.exists() and not args.resume:
        raise ValueError("existing grounded intake output requires --resume")

    real_data_path = None if args.without_real_data else args.real_data_file
    public_literature_path = (
        None if args.without_public_literature else args.public_literature_file
    )
    source_refresh = _refresh_grounded_sources(
        real_data_path=real_data_path,
        public_literature_path=public_literature_path,
        refresh_real_data=args.refresh_real_data,
        refresh_public_literature=args.refresh_public_literature,
        approve_network=args.approve_network,
        timeout=args.refresh_timeout,
        resume=args.resume,
    )
    source_refresh_payload = _validate_grounded_source_refresh({"performed": source_refresh})
    real_data = _load_grounded_portfolio_bundle(
        real_data_path,
        expected_schema="bioprism-neurosurgery-real/0.1",
    )
    public_literature = _load_grounded_portfolio_bundle(
        public_literature_path,
        expected_schema="bioprism-neurosurgery-public-literature/0.1",
    )
    real_data_query = _load_grounded_real_data_query(args.real_data_query_file)
    public_literature_query = _load_grounded_public_literature_query(args.public_literature_query_file)
    case_asset_manifest = _load_grounded_case_asset_manifest(args.case_asset_manifest)
    case_asset_manifest_query = _load_grounded_real_data_query(args.case_asset_manifest_query)
    if case_asset_manifest_query is not None and case_asset_manifest is None:
        raise ValueError("--case-asset-manifest-query requires --case-asset-manifest")
    if real_data_query is not None and real_data is None:
        raise ValueError("--real-data-query-file requires the real-glioma plane")
    if public_literature_query is not None and public_literature is None:
        raise ValueError("--public-literature-query-file requires the public-literature plane")
    expected_source_paths = {
        "real_data_file": None if args.without_real_data else args.real_data_file,
        "public_literature_file": None if args.without_public_literature else args.public_literature_file,
    }
    if args.real_data_query_file is not None:
        expected_source_paths["real_data_query_file"] = args.real_data_query_file
    if args.public_literature_query_file is not None:
        expected_source_paths["public_literature_query_file"] = args.public_literature_query_file
    if args.case_asset_manifest is not None:
        expected_source_paths["case_asset_manifest"] = args.case_asset_manifest
    if args.case_asset_manifest_query is not None:
        expected_source_paths["case_asset_manifest_query"] = args.case_asset_manifest_query
    prior_intake = None if prior_store is None else prior_store["intake"]
    if prior_store is not None and not source_refresh:
        source_refresh_payload = prior_store["source_refresh"]
    if prior_store is not None:
        question_digest = hashlib.sha256(args.question.encode("utf-8")).hexdigest()
        if prior_store.get("question_digest") != question_digest:
            raise ValueError("--resume question does not match the persisted intake")
        if prior_store.get("provider") != args.provider or prior_store.get("model") != args.model:
            raise ValueError("--resume provider/model does not match the persisted intake")
        if prior_store.get("routed_specialty") != prior_intake.get("routed_specialty"):
            raise ValueError("persisted intake specialty identity is inconsistent")
        if dict(prior_store.get("source_paths", {})) != expected_source_paths:
            raise ValueError("--resume source selection does not match the persisted intake")
        if args.specialty is not None and prior_intake.get("routed_specialty") != args.specialty:
            raise ValueError("--resume specialty does not match the persisted intake")
    freshness = None
    if args.freshness_as_of is not None:
        freshness = {
            "as_of": args.freshness_as_of,
            "max_age_days": args.freshness_max_age_days,
        }
    controls = {
        "max_passes": args.max_passes,
        "max_follow_ups_per_pass": args.max_follow_ups_per_pass,
        "max_output_tokens": args.max_output_tokens,
        "max_hits": args.max_hits,
        "max_chars": args.max_chars,
        "tool_loop": args.tool_loop,
        "max_tool_turns": args.max_tool_turns,
        "max_tool_calls": args.max_tool_calls,
        "include_abstracts": not args.no_abstracts,
        "freshness": freshness,
        "case_asset_manifest": None if case_asset_manifest is None else content_digest(case_asset_manifest),
        "case_asset_manifest_query": case_asset_manifest_query,
    }
    if prior_store is not None:
        prior_controls = prior_store.get("controls")
        if not isinstance(prior_controls, Mapping):
            raise ValueError("persisted intake controls are invalid")
        for key, value in controls.items():
            if key == "max_passes":
                previous = prior_controls.get(key)
                if not isinstance(previous, int) or value < previous:
                    raise ValueError("--resume max-passes cannot shrink the persisted budget")
            elif prior_controls.get(key) != value:
                raise ValueError(f"--resume control {key} does not match the persisted intake")
    command = _parse_mcp_command(args.mcp_command)
    runtime, onboarding = _runtime_with_provider(args)
    session = onboarding.start_session()
    try:
        client = client_factory(command, cwd=args.mcp_cwd, timeout=args.mcp_timeout)
        with client:
            agent = LocalNeurosurgicalAgent(client)
            intake = agent.grounded_research_intake(
                args.question,
                runtime,
                args.provider,
                args.model,
                specialty=args.specialty,
                real_glioma_data=real_data,
                public_literature=public_literature,
                case_asset_manifest=case_asset_manifest,
                case_asset_manifest_query=case_asset_manifest_query,
                approve_provider_call=True,
                max_passes=args.max_passes,
                max_follow_ups_per_pass=args.max_follow_ups_per_pass,
                max_output_tokens=args.max_output_tokens,
                max_hits=args.max_hits,
                max_chars=args.max_chars,
                include_abstracts=not args.no_abstracts,
                freshness=freshness,
                real_data_query=real_data_query,
                public_literature_query=public_literature_query,
                tool_loop=args.tool_loop,
                max_tool_turns=args.max_tool_turns,
                max_tool_calls=args.max_tool_calls,
                real_resume_from=(
                    None
                    if prior_intake is None or not isinstance(prior_intake.get("portfolio"), Mapping)
                    else prior_intake["portfolio"].get("real_data_loop")
                ),
                public_resume_from=(
                    None
                    if prior_intake is None or not isinstance(prior_intake.get("portfolio"), Mapping)
                    else prior_intake["portfolio"].get("public_literature_loop")
                ),
            )
        persisted_store = _persist_grounded_intake_store(
            str(output_path),
            intake=intake,
            provider=args.provider,
            model=args.model,
            source_paths=expected_source_paths,
            controls=controls,
            source_refresh=source_refresh_payload,
        )
        return {
            "schema": CLI_SCHEMA,
            "intake_schema": GROUNDED_INTAKE_CLI_SCHEMA,
            "command": "grounded-autopilot",
            "intake": intake,
            "source_refresh": source_refresh_payload,
            "source_paths": {
                **expected_source_paths,
            },
            "persistence": {
                "store": str(output_path),
                "store_digest": persisted_store["store_digest"],
                "resume_requested": args.resume,
                "resumed": prior_intake is not None,
                "retention": "caller_owned_grounded_answers_and_claims; no_credentials_or_patient_data",
            },
            "provider_status": runtime.provider_status(args.provider),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": True,
                "network": args.provider == "ollama" or bool(source_refresh),
                "source_refresh_network_approved": bool(source_refresh),
                "clinical_actions_authorized": False,
                "human_review_required": True,
            },
            "secret_material": "never_returned",
        }
    finally:
        session.close()


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
    if args.capability is not None and args.automatic:
        raise ValueError("--capability requires an explicit --domain")
    if args.approve_capability and args.capability is None:
        raise ValueError("--approve-capability requires --capability")
    if args.domain_tool_domain and not (args.activate_domain_tools or args.approve_domain_tool):
        raise ValueError("--domain-tool-domain requires --activate-domain-tools or --approve-domain-tool")
    if args.domain_tool_bindings_file and args.domain_tool_domain:
        raise ValueError("--domain-tool-domain cannot be combined with --domain-tool-bindings-file")
    if args.domain_tool_bindings_file and (
        args.activate_domain_tools or args.approve_domain_tool or args.resume_activation
    ):
        raise ValueError("--domain-tool-bindings-file cannot be combined with curated activation flags")
    activation_store, loaded_activation_state, activation_resumed = _load_activation_store(args)
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
    evidence = _load_evidence_file(args.evidence_file)
    launch_admission = _load_launch_admission_file(args.launch_admission_file)
    persisted_candidates = _persisted_candidate_args(args) if args.use_inventory else ()
    runtime, onboarding = _runtime_with_provider(args)
    _preflight_cli_launch_admission(
        launch_admission,
        runtime,
        task=args.task,
        domain=args.domain,
        automatic=args.automatic,
        hints=tuple(args.hint or ()),
        max_domains=args.max_domains,
        allow_cross_domain=not args.single_domain,
        semantic_routing=args.semantic_routing,
    )
    session = onboarding.start_session(ttl_seconds=args.ttl_seconds)
    health_ledger = None
    learning_ledger = None
    learning_memory = None
    execution_journal = None
    activation_state_after: Mapping[str, Any] | None = None
    activation_persisted = False
    tool_surface: dict[str, Any] = {
        "mode": "not_requested",
        "exposed_count": 0,
        "catalogue_digest": None,
        "exposed_tools": [],
        "domain_binding": {"requested": False},
        "authority": "not_applicable",
        "retention": "tool_names_and_schema_digest_only",
    }
    try:
        if args.health_store is not None:
            health_ledger = ProviderHealthLedger(args.health_store)
        if args.learning_store is not None:
            learning_ledger = SQLiteBrainLearningLedger(args.learning_store)
        learning_memory = _open_cli_memory(
            args,
            learning_requested=args.learning_mode != "off",
        )
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
            if loaded_activation_state is not None:
                agent_kwargs["activation"] = AutonomousCapabilityActivation(
                    state=loaded_activation_state
                )
            agent = AutonomousAgent(_McpWorkspace(client), runtime, **agent_kwargs)
            domain_activation_requested = bool(
                args.activate_domain_tools or args.approve_domain_tool
            )
            binding_file_requested = args.domain_tool_bindings_file is not None
            domain_registry_requested = (
                domain_activation_requested or activation_resumed or binding_file_requested
            )
            provider_tools, tool_catalogue = (
                _mcp_provider_tools(
                    client,
                    allow_tools=tuple(args.allow_mcp_tool or ()),
                    deny_tools=tuple(args.deny_mcp_tool or ()),
                )
                if args.execution_mode in {"tool_loop", "mission"} or domain_registry_requested
                else ((), None)
            )
            if args.execution_mode in {"tool_loop", "mission"} or domain_registry_requested:
                assert tool_catalogue is not None
                tool_surface = {
                    "mode": "live_mcp",
                    "exposed_count": len(tool_catalogue.definitions),
                    "catalogue_digest": tool_catalogue.digest,
                    "exposed_tools": sorted(
                        definition.name for definition in tool_catalogue.definitions
                    ),
                    "allowlist": sorted(set(args.allow_mcp_tool or ())),
                    "denylist": sorted(set(args.deny_mcp_tool or ())),
                    "authority": "caller_approved_only",
                    "retention": "tool_names_and_schema_digest_only",
                }
            domain_binding = None
            if binding_file_requested:
                assert tool_catalogue is not None
                domain_binding = _register_domain_tool_bindings_file(
                    agent,
                    tool_catalogue,
                    args.domain_tool_bindings_file,
                    approve_mission_dispatch=args.approve_mission_dispatch,
                )
                tool_surface["domain_binding"] = domain_binding
                tool_surface["mode"] = "domain_registry"
            elif domain_activation_requested:
                assert tool_catalogue is not None
                domain_binding = _activate_domain_tools(
                    agent,
                    tool_catalogue,
                    domains=_domain_tool_domains(args),
                    activate=args.activate_domain_tools,
                    approved_tools=tuple(args.approve_domain_tool or ()),
                )
                assert domain_binding is not None
                tool_surface["domain_binding"] = domain_binding
                tool_surface["mode"] = "domain_registry"
            elif activation_resumed:
                assert tool_catalogue is not None
                domain_binding = _rehydrate_domain_tools(
                    agent,
                    tool_catalogue,
                    previous_state=loaded_activation_state.to_dict(),
                )
                tool_surface["domain_binding"] = domain_binding
                tool_surface["mode"] = "domain_registry"
            activation_stale = bool(
                activation_resumed
                and isinstance(domain_binding, Mapping)
                and domain_binding.get("activation_status") == "stale"
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
            if learning_memory is not None:
                common["memory"] = learning_memory
            if evidence is not None:
                common["evidence"] = evidence
            if provider_tools and not domain_registry_requested:
                common["provider_tools"] = provider_tools
                common["tool_loop_options"] = {
                    "authorize_and_execute": _cli_tool_authorizer(
                        client,
                        catalogue=tool_catalogue,
                        approve_mission_dispatch=args.approve_mission_dispatch,
                        read_only_tools=_registered_tool_posture(agent),
                    ),
                }
            elif domain_registry_requested and args.execution_mode in {"tool_loop", "mission"}:
                registered_names = () if domain_binding is None else tuple(
                    domain_binding.get("registered_tools", ())
                )
                common["tool_loop_options"] = {
                    "authorize_and_execute": _cli_tool_authorizer(
                        client,
                        catalogue=tool_catalogue,
                        approve_mission_dispatch=args.approve_mission_dispatch,
                        allowed_tools=registered_names,
                        read_only_tools=_registered_tool_posture(agent),
                    ),
                }
            if activation_stale:
                # A restarted process must not attempt a provider call against an empty or
                # changed domain-tool surface.  Returning a structured no-op keeps the CLI
                # restart-safe and lets the operator inspect the redacted stale binding before
                # explicitly re-activating the fresh catalogue.  In particular, do not feed a
                # provider's stale tool call into the runtime with zero advertised tools: that
                # would turn a safe activation refusal into an opaque provider transport error.
                result = {
                    "status": "activation_stale",
                    "execution": "not_started",
                    "provider_call": False,
                    "tool_calls": 0,
                    "reason": "domain_tool_activation_requires_fresh_approval",
                    "retention": "activation_digests_and_status_only; no_task_prompt_or_provider_payloads",
                    "secret_material": "never_returned",
                }
            elif args.automatic:
                automatic_options = {
                    **common,
                    "learning_mode": args.learning_mode,
                    "hints": tuple(args.hint or ()),
                    "max_domains": args.max_domains,
                    "allow_cross_domain": not args.single_domain,
                    "semantic_routing": args.semantic_routing,
                    "planning_mode": args.planning_mode,
                    "planning_run_id": args.planning_run_id,
                    "planning_max_output_tokens": args.planning_max_output_tokens,
                    "workflow_execution": args.workflow_execution,
                    "workflow_checkpoint": workflow_checkpoint,
                    "workflow_retry_blocked": args.workflow_retry_blocked,
                    "workflow_max_stage_calls": args.workflow_max_stage_calls,
                }
                if launch_admission is None:
                    result = agent.run_auto(**automatic_options)
                else:
                    result = agent.run_auto_with_launch_admission(
                        launch_admission=launch_admission,
                        **automatic_options,
                    )
            else:
                if args.learning_mode == "online":
                    common["learn"] = True
                    common["bandit_state"] = _cli_learning_state(
                        agent,
                        domain=args.domain,
                        capability=args.capability,
                    )
                if args.capability is not None:
                    capability_options = dict(common)
                    capability_options.pop("capability", None)
                    if launch_admission is None:
                        result = agent.run_capability(
                            **capability_options,
                            domain=args.domain,
                            capability=args.capability,
                            approve_capability=args.approve_capability,
                        )
                    else:
                        result = agent.run_capability_with_launch_admission(
                            **capability_options,
                            domain=args.domain,
                            capability=args.capability,
                            approve_capability=args.approve_capability,
                            launch_admission=launch_admission,
                        )
                else:
                    if launch_admission is None:
                        result = agent.run(**common, domain=args.domain)
                    else:
                        result = agent.run_with_launch_admission(
                            **common,
                            domain=args.domain,
                            launch_admission=launch_admission,
                        )
            if activation_store is not None:
                agent.save_activation(activation_store)
                activation_state_after = agent.activation_state()
                activation_persisted = True
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
            "launch_admission": _launch_admission_projection(launch_admission),
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
            "tool_surface": tool_surface,
            "activation_persistence": _activation_persistence_projection(
                activation_store,
                resumed=activation_resumed,
                persisted=activation_persisted,
                state=activation_state_after,
            ),
            "credential_session": session.status().to_dict(),
            "authorization": {
                "provider_call_approved": args.approve_provider_call,
                "model_discovery_approved": args.approve_provider_call if args.discover_models else False,
                "capability_approved": args.approve_capability,
                "mission_dispatch_approved": args.approve_mission_dispatch,
            },
            "state_persistence": {
                "health_store_configured": health_ledger is not None,
                "learning_store_configured": learning_ledger is not None,
                "memory_store_configured": learning_memory is not None,
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
        if learning_memory is not None:
            learning_memory.close()
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

    action_plan = subparsers.add_parser(
        "action-plan",
        help="compile a digest-bound provider-free action plan for review",
    )
    action_plan.add_argument("--task", required=True)
    action_plan.add_argument("--domain", action="append", choices=AUTONOMOUS_DOMAINS, help="explicit domain; repeatable")
    action_plan.add_argument("--all-domains", action="store_true", help="compile one plan for every reviewed domain")
    action_plan.add_argument("--hint", action="append", default=[], help="routing hint; repeatable")
    action_plan.add_argument("--max-domains", type=int, default=3, help="maximum automatic route fan-out")
    action_plan.add_argument("--single-domain", action="store_true", help="prevent automatic cross-domain fan-out")

    action_status = subparsers.add_parser(
        "action-admission-status",
        help="inspect a durable metadata-only action review queue",
    )
    action_status.add_argument("--admission-store", required=True, help="canonical action-admission snapshot path")

    action_submit = subparsers.add_parser(
        "action-admission-submit",
        help="submit one serialized provider-free action plan to the review queue",
    )
    action_submit.add_argument("--admission-store", required=True, help="canonical action-admission snapshot path")
    action_submit.add_argument("--plan-file", required=True, help="serialized action plan or action-plan command output")
    action_submit.add_argument("--action-id", required=True, help="stable caller-owned action identity")

    action_review = subparsers.add_parser(
        "action-admission-review",
        help="append an authorized, digest-fenced review decision to an action queue",
    )
    action_review.add_argument("--admission-store", required=True, help="canonical action-admission snapshot path")
    action_review.add_argument("--action-id", required=True)
    action_review.add_argument("--authorization-digest", required=True, help="external reviewer authorization digest; never a provider key")
    action_review.add_argument("--expected-record-digest", required=True, help="exact queue row digest shown to the reviewer")
    action_review.add_argument("--approve-gate", action="append", choices=AUTONOMOUS_TASK_DECISION_APPROVALS, default=[], help="approve one named gate; repeatable")
    action_review.add_argument("--deny-gate", action="append", choices=AUTONOMOUS_TASK_DECISION_APPROVALS, default=[], help="deny one named gate; repeatable")
    action_review.add_argument("--reviewed", action="store_true", help="acknowledge retained task-decision review reasons")
    action_review.add_argument("--reason", default=None, help="bounded operator reason; only its digest is persisted")

    action_handoff = subparsers.add_parser(
        "action-admission-handoff",
        help="emit a reviewed action's downstream-only handoff",
    )
    action_handoff.add_argument("--admission-store", required=True, help="canonical action-admission snapshot path")
    action_handoff.add_argument("--action-id", required=True)
    action_handoff.add_argument("--domain", action="append", choices=AUTONOMOUS_DOMAINS, default=[], help="optional admitted subset; repeatable")

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
    provider_parent.add_argument(
        "--allow-mcp-tool",
        action="append",
        default=[],
        help="restrict model-visible MCP tools to these names; repeatable",
    )
    provider_parent.add_argument(
        "--deny-mcp-tool",
        action="append",
        default=[],
        help="remove these names from the model-visible MCP tool surface; repeatable",
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

    activation_status = subparsers.add_parser(
        "activation-status",
        help="read a persisted domain activation snapshot without contacting a provider or MCP",
    )
    activation_status.add_argument("--activation-store", required=True, help="redacted activation snapshot path")

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
    run.add_argument(
        "--memory-store",
        default=None,
        help="persist digest-only episodic memory for recall and evaluator-gated learning",
    )
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
    run.add_argument(
        "--evidence-file",
        default=None,
        help="bounded caller/evaluator evidence JSON object for an online learning decision",
    )
    run.add_argument("--model-limit", type=int, default=64, help="maximum provider inventory rows to inspect when discovering")
    run.add_argument("--model-capability", action="append", default=[], help="declared capability for every model candidate")
    run.add_argument("--required-model-capability", action="append", default=[], help="capability required by this run")
    run.add_argument("--context-window-tokens", type=int, default=_DEFAULT_CONTEXT_WINDOW)
    run.add_argument("--model-max-output-tokens", type=int, default=_DEFAULT_MAX_OUTPUT)
    run.add_argument("--quality", type=float, default=0.5)
    run.add_argument("--reliability", type=float, default=0.5)
    run.add_argument("--latency-ms", type=int, default=1_000)
    run.add_argument("--cost-per-million-tokens", type=int, default=0)
    run.add_argument("--capability", default=None, help="focused reviewed domain capability; dispatches through run_capability")
    run.add_argument("--approve-capability", action="store_true", help="approve a capability contract that requires human review")
    run.add_argument("--execution-mode", choices=("provider", "tool_loop", "mission"), default="provider")
    run.add_argument("--max-steps", type=int, default=8)
    run.add_argument("--requested-output-tokens", type=int, default=2_048)
    run.add_argument(
        "--activate-domain-tools",
        action="store_true",
        help="plan and activate all exact curated read-only MCP bindings for the selected scope",
    )
    run.add_argument(
        "--approve-domain-tool",
        action="append",
        default=[],
        help="activate one exact curated read-only MCP binding; repeatable and implies activation",
    )
    run.add_argument(
        "--domain-tool-domain",
        action="append",
        choices=AUTONOMOUS_DOMAINS,
        default=[],
        help="domain scope for curated MCP binding activation; repeatable (default: task scope/all)",
    )
    run.add_argument(
        "--activation-store",
        default=None,
        help="optional atomic redacted activation snapshot path",
    )
    run.add_argument(
        "--resume-activation",
        action="store_true",
        help="rehydrate approved domain bindings from --activation-store after live catalogue validation",
    )
    run.add_argument(
        "--domain-tool-bindings-file",
        default=None,
        help="strict JSON file declaring caller-owned domains, capabilities, and risk posture for live MCP tools",
    )
    run.add_argument("--run-id", default=None)
    run.add_argument(
        "--launch-admission-file",
        default=None,
        help="digest-verified caller approval JSON; checked before credential collection and dispatch",
    )
    run.add_argument("--approve-provider-call", action="store_true", help="authorize provider invocation")
    run.add_argument("--approve-mission-dispatch", action="store_true", help="authorize mission effects")
    _add_credential_arguments(run)

    refresh_literature = subparsers.add_parser(
        "refresh-public-literature",
        help="refresh six bounded real PubMed specialty lanes without an API key",
    )
    refresh_literature.add_argument(
        "--output",
        default="data/neurosurgery/neurosurgical_public_literature_snapshot.json",
        help="same-directory atomic output snapshot path",
    )
    refresh_literature.add_argument(
        "--per-specialty-limit",
        type=int,
        default=10,
        help="maximum PubMed records requested per specialty lane (1-50)",
    )
    refresh_literature.add_argument(
        "--timeout",
        type=float,
        default=_DEFAULT_TIMEOUT,
        help="per-request NCBI timeout in seconds (1-120)",
    )
    refresh_literature.add_argument(
        "--approve-network",
        action="store_true",
        help="explicitly authorize public NCBI network retrieval; no credential is accepted",
    )

    refresh_real = subparsers.add_parser(
        "refresh-real-glioma",
        help="refresh aggregate real glioma registry, genomic, portal, guideline, and PubMed metadata",
    )
    refresh_real.add_argument(
        "--output",
        default="data/neurosurgery/glioma_public_snapshot.json",
        help="same-directory atomic output snapshot path",
    )
    refresh_real.add_argument("--gdc-project-id", action="append", default=None, help="TCGA project ID; repeat for a broader real population")
    refresh_real.add_argument("--trial-page-size", type=int, default=5, help="maximum ClinicalTrials.gov studies (1-100)")
    refresh_real.add_argument("--portal-study-id", action="append", default=None, help="public cBioPortal study ID; repeatable")
    refresh_real.add_argument("--portal-study-limit", type=int, default=7, help="number of selected cBioPortal studies (1-100)")
    refresh_real.add_argument("--pubmed-limit", type=int, default=20, help="maximum glioma PubMed records (1-50)")
    refresh_real.add_argument("--pubmed-term", default=DEFAULT_PUBMED_TERM, help="bounded PubMed search expression")
    refresh_real.add_argument("--pubmed-source-id", default=DEFAULT_PUBMED_SOURCE_ID, help="stable lowercase PubMed provenance ID")
    refresh_real.add_argument("--timeout", type=float, default=_DEFAULT_TIMEOUT, help="per-request timeout in seconds (1-120)")
    refresh_real.add_argument(
        "--approve-network",
        action="store_true",
        help="explicitly authorize public registry/GDC/cBioPortal/PubMed retrieval; no credential is accepted",
    )

    grounded = subparsers.add_parser(
        "grounded-portfolio",
        parents=[provider_parent],
        help="run bounded real-glioma and PubMed research loops through a no-key local provider",
    )
    grounded.set_defaults(provider="ollama")
    grounded.add_argument("--mcp-command", required=True, help="MCP executable and arguments; no shell is invoked")
    grounded.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    grounded.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)
    grounded.add_argument("--question", required=True, help="research question; retained only through its digest in the store")
    grounded.add_argument(
        "--specialty",
        choices=("glioma", "cranial_base", "craniosynostosis", "encephalocele", "spina_bifida", "chiari_malformation"),
        default="glioma",
    )
    grounded.add_argument(
        "--real-data-file",
        default="data/neurosurgery/glioma_extended_snapshot.json",
        help="validated non-synthetic glioma snapshot JSON (default: extended TCGA-GBM + TCGA-LGG)",
    )
    grounded.add_argument(
        "--public-literature-file",
        default="data/neurosurgery/neurosurgical_public_literature_snapshot.json",
        help="validated non-synthetic six-specialty PubMed snapshot JSON",
    )
    grounded.add_argument(
        "--refresh-real-data",
        action="store_true",
        help="refresh the real glioma snapshot from allow-listed public sources before running",
    )
    grounded.add_argument(
        "--refresh-public-literature",
        action="store_true",
        help="refresh the six-specialty PubMed snapshot from NCBI before running",
    )
    grounded.add_argument(
        "--approve-network",
        action="store_true",
        help="explicitly authorize credentialless public-source refresh (required by refresh flags)",
    )
    grounded.add_argument(
        "--refresh-timeout",
        type=float,
        default=_DEFAULT_TIMEOUT,
        help="per-request timeout for an opt-in public-source refresh (1-120 seconds)",
    )
    grounded.add_argument(
        "--real-data-query-file",
        default=None,
        help="optional JSON object with bounded real-data query facets applied to the glioma plane",
    )
    grounded.add_argument(
        "--public-literature-query-file",
        default=None,
        help="optional JSON object with bounded PubMed facets applied to the public-literature plane",
    )
    grounded.add_argument(
        "--case-asset-manifest",
        default=None,
        help="optional real de-identified case-asset manifest JSON (metadata only; bytes are never opened)",
    )
    grounded.add_argument(
        "--case-asset-manifest-query",
        default=None,
        help="optional bounded JSON query selecting case-asset kinds and review-item budget",
    )
    grounded.add_argument("--without-real-data", action="store_true", help="run only the public-literature plane")
    grounded.add_argument("--without-public-literature", action="store_true", help="run only the real-glioma plane")
    grounded.add_argument("--model", default="llama3.1", help="local model identifier (for example llama3.1)")
    grounded.add_argument("--max-passes", type=int, default=3, help="maximum passes per source plane (1-8)")
    grounded.add_argument("--max-follow-ups-per-pass", type=int, default=4, help="maximum unknown-derived follow-ups per pass (0-8)")
    grounded.add_argument("--max-output-tokens", type=int, default=2_048)
    grounded.add_argument("--max-hits", type=int, default=32)
    grounded.add_argument("--max-chars", type=int, default=24_000)
    grounded.add_argument(
        "--tool-loop",
        action="store_true",
        help="allow the approved local model to use bounded read-only snapshot search, coverage, cohort, and reconciliation views",
    )
    grounded.add_argument("--max-tool-turns", type=int, default=4, help="maximum local-model tool-loop turns (1-8)")
    grounded.add_argument("--max-tool-calls", type=int, default=8, help="maximum snapshot-search calls (1-32)")
    grounded.add_argument("--no-abstracts", action="store_true", help="exclude bounded PubMed abstracts from local-model context")
    grounded.add_argument("--freshness-as-of", default=None, help="UTC source-age clock, YYYY-MM-DDTHH:MM:SSZ")
    grounded.add_argument("--freshness-max-age-days", type=int, default=None)
    grounded.add_argument("--portfolio-output", default="work/grounded-research-portfolio.json", help="atomic digest-bound portfolio ledger path")
    grounded.add_argument("--resume", action="store_true", help="resume pending work from --portfolio-output")
    grounded.add_argument("--approve-provider-call", action="store_true", help="authorize local/loopback model invocation")

    autopilot = subparsers.add_parser(
        "grounded-autopilot",
        parents=[provider_parent],
        help="route a neurosurgical question, gate real evidence, and run bounded no-key research",
    )
    autopilot.set_defaults(provider="ollama")
    autopilot.add_argument("--mcp-command", required=True, help="MCP executable and arguments; no shell is invoked")
    autopilot.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    autopilot.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)
    autopilot.add_argument("--question", required=True, help="free-text research question")
    autopilot.add_argument(
        "--specialty",
        choices=("glioma", "cranial_base", "craniosynostosis", "encephalocele", "spina_bifida", "chiari_malformation"),
        default=None,
        help="optional specialty hint; omit to use deterministic vocabulary routing",
    )
    autopilot.add_argument(
        "--real-data-file",
        default="data/neurosurgery/glioma_extended_snapshot.json",
        help="validated non-synthetic glioma snapshot JSON (default: extended TCGA-GBM + TCGA-LGG)",
    )
    autopilot.add_argument(
        "--public-literature-file",
        default="data/neurosurgery/neurosurgical_public_literature_snapshot.json",
        help="validated non-synthetic six-specialty PubMed snapshot JSON",
    )
    autopilot.add_argument(
        "--refresh-real-data",
        action="store_true",
        help="refresh the real glioma snapshot from allow-listed public sources before routing",
    )
    autopilot.add_argument(
        "--refresh-public-literature",
        action="store_true",
        help="refresh the six-specialty PubMed snapshot from NCBI before routing",
    )
    autopilot.add_argument(
        "--approve-network",
        action="store_true",
        help="explicitly authorize credentialless public-source refresh (required by refresh flags)",
    )
    autopilot.add_argument(
        "--refresh-timeout",
        type=float,
        default=_DEFAULT_TIMEOUT,
        help="per-request timeout for an opt-in public-source refresh (1-120 seconds)",
    )
    autopilot.add_argument(
        "--real-data-query-file",
        default=None,
        help="optional JSON object with bounded real-data query facets applied to the glioma plane",
    )
    autopilot.add_argument(
        "--public-literature-query-file",
        default=None,
        help="optional JSON object with bounded PubMed facets applied to the public-literature plane",
    )
    autopilot.add_argument(
        "--case-asset-manifest",
        default=None,
        help="optional real de-identified case-asset manifest JSON (metadata only; bytes are never opened)",
    )
    autopilot.add_argument(
        "--case-asset-manifest-query",
        default=None,
        help="optional bounded JSON query selecting case-asset kinds and review-item budget",
    )
    autopilot.add_argument("--without-real-data", action="store_true", help="withhold the real-glioma plane")
    autopilot.add_argument("--without-public-literature", action="store_true", help="withhold the PubMed plane")
    autopilot.add_argument("--model", default="llama3.1", help="local model identifier (for example llama3.1)")
    autopilot.add_argument("--max-passes", type=int, default=3, help="maximum passes per source plane (1-8)")
    autopilot.add_argument("--max-follow-ups-per-pass", type=int, default=4, help="maximum unknown-derived follow-ups per pass (0-8)")
    autopilot.add_argument("--max-output-tokens", type=int, default=2_048)
    autopilot.add_argument("--max-hits", type=int, default=32)
    autopilot.add_argument("--max-chars", type=int, default=24_000)
    autopilot.add_argument(
        "--tool-loop",
        action="store_true",
        help="allow the approved local model to use bounded read-only snapshot search, coverage, cohort, and reconciliation views",
    )
    autopilot.add_argument("--max-tool-turns", type=int, default=4, help="maximum local-model tool-loop turns (1-8)")
    autopilot.add_argument("--max-tool-calls", type=int, default=8, help="maximum snapshot-search calls (1-32)")
    autopilot.add_argument("--no-abstracts", action="store_true", help="exclude bounded PubMed abstracts from local-model context")
    autopilot.add_argument("--freshness-as-of", default=None, help="UTC source-age clock, YYYY-MM-DDTHH:MM:SSZ")
    autopilot.add_argument("--freshness-max-age-days", type=int, default=None)
    autopilot.add_argument(
        "--intake-output",
        default="work/grounded-research-intake.json",
        help="atomic digest-bound routed-intake checkpoint path",
    )
    autopilot.add_argument("--resume", action="store_true", help="resume pending work from --intake-output")
    autopilot.add_argument("--approve-provider-call", action="store_true", help="authorize local/loopback model invocation")

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
    batch_run.add_argument(
        "--launch-admission-file",
        default=None,
        help="digest-verified caller approval JSON; checked before credential collection and dispatch",
    )
    batch_run.add_argument("--model", action="append", default=[], help="shared candidate model; repeatable")
    batch_run.add_argument("--discover-models", action="store_true", help="discover selectable models through the approved provider inventory endpoint")
    batch_run.add_argument("--use-inventory", action="store_true", help="rehydrate selectable candidates from --inventory-store")
    batch_run.add_argument("--inventory-store", default=None, help="digest-bound metadata-only inventory store")
    batch_run.add_argument("--health-store", default=None, help="persist provider/model health observations")
    batch_run.add_argument("--learning-store", default=None, help="persist value-only online-learning state")
    batch_run.add_argument(
        "--memory-store",
        default=None,
        help="persist digest-only episodic memory for recall and evaluator-gated learning",
    )
    batch_run.add_argument("--execution-store", default=None, help="persist hash-chained metadata-only execution checkpoints")
    batch_run.add_argument("--execution-mode", choices=("provider", "tool_loop", "mission"), default=None)
    batch_run.add_argument("--max-steps", type=int, default=None)
    batch_run.add_argument("--requested-output-tokens", type=int, default=None)
    batch_run.add_argument(
        "--activate-domain-tools",
        action="store_true",
        help="plan and activate all exact curated read-only MCP bindings across all domains",
    )
    batch_run.add_argument(
        "--approve-domain-tool",
        action="append",
        default=[],
        help="activate one exact curated read-only MCP binding; repeatable and implies activation",
    )
    batch_run.add_argument(
        "--domain-tool-domain",
        action="append",
        choices=AUTONOMOUS_DOMAINS,
        default=[],
        help="domain scope for curated MCP binding activation; repeatable (default: all)",
    )
    batch_run.add_argument(
        "--activation-store",
        default=None,
        help="optional atomic redacted activation snapshot path",
    )
    batch_run.add_argument(
        "--resume-activation",
        action="store_true",
        help="rehydrate approved domain bindings from --activation-store after live catalogue validation",
    )
    batch_run.add_argument(
        "--domain-tool-bindings-file",
        default=None,
        help="strict JSON file declaring caller-owned domains, capabilities, and risk posture for live MCP tools",
    )
    batch_run.add_argument("--learning-mode", choices=("off", "online", "trajectory"), default=None, help="automatic route learning mode")
    batch_run.add_argument(
        "--evidence-file",
        default=None,
        help="bounded caller/evaluator evidence JSON object shared by eligible batch items",
    )
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
    batch_run.add_argument("--approve-capability", action="store_true", help="approve capability contracts requiring human review")
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
        elif args.command == "action-plan":
            payload = _action_plan(args)
        elif args.command == "action-admission-status":
            payload = _action_admission_status(args)
        elif args.command == "action-admission-submit":
            payload = _action_admission_submit(args)
        elif args.command == "action-admission-review":
            payload = _action_admission_review(args)
        elif args.command == "action-admission-handoff":
            payload = _action_admission_handoff(args)
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
        elif args.command == "activation-status":
            payload = _activation_status(args)
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
        elif args.command == "refresh-public-literature":
            payload = _refresh_public_literature(args)
        elif args.command == "refresh-real-glioma":
            payload = _refresh_real_glioma(args)
        elif args.command == "grounded-portfolio":
            payload = _grounded_portfolio(args, client_factory=client_factory)
        elif args.command == "grounded-autopilot":
            payload = _grounded_autopilot(args, client_factory=client_factory)
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


__all__ = [
    "CLI_SCHEMA",
    "GROUNDED_PORTFOLIO_CLI_SCHEMA",
    "GROUNDED_INTAKE_CLI_SCHEMA",
    "GROUNDED_INTAKE_STORE_SCHEMA",
    "PUBLIC_LITERATURE_REFRESH_CLI_SCHEMA",
    "REAL_DATA_REFRESH_CLI_SCHEMA",
    "main",
]
