"""Operator-facing command line boundary for the autonomous SDK.

The SDK is intentionally embeddable, but a useful autonomous system also needs a small,
well-defined process boundary.  This module provides that boundary without moving secrets into
the brain or into MCP:

* ``catalogue`` and ``evidence-plan`` are provider-free inspection commands;
* ``route`` exposes deterministic routing evidence without invoking a model;
* ``provider-status`` and ``onboard`` implement the redacted BYOK lifecycle; and
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
from .client import Client
from .errors import SdkError
from .evaluators import builtin_autonomous_domain_evaluator_profiles
from .llm_runtime import (
    LLMRuntime,
    ModelCandidate,
    ModelCatalogue,
    ProviderOnboarding,
    anthropic_provider,
    openai_compatible_provider,
    openai_provider,
)


CLI_SCHEMA = "aurora-autonomous-cli/0.1"
_DEFAULT_CONTEXT_WINDOW = 128_000
_DEFAULT_MAX_OUTPUT = 4_096
_DEFAULT_TIMEOUT = 30.0


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
    onboarding.register_provider(_provider_config(args))
    return runtime, onboarding


def _candidate_args(args: argparse.Namespace) -> tuple[ModelCandidate, ...]:
    models = tuple(args.model or ())
    if not models:
        raise ValueError("at least one --model is required")
    capabilities = tuple(args.model_capability or ())
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


def _credential_reader(reader: Callable[[str], str] | None) -> Callable[[str], str]:
    return reader if reader is not None else getpass.getpass


def _collect_credentials(
    args: argparse.Namespace,
    session: Any,
    *,
    environ: Mapping[str, str],
    reader: Callable[[str], str] | None,
) -> None:
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
        provider_status = session.provider_statuses()[0]
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
    command = _parse_mcp_command(args.mcp_command)
    candidates = _candidate_args(args)
    runtime, onboarding = _runtime_with_provider(args)
    catalogue = ModelCatalogue(candidates)
    session = onboarding.start_session(ttl_seconds=args.ttl_seconds)
    try:
        _collect_credentials(
            args,
            session,
            environ=environ,
            reader=reader,
        )
        client = client_factory(
            command,
            cwd=args.mcp_cwd,
            timeout=args.mcp_timeout,
        )
        with client:
            agent = AutonomousAgent(
                _McpWorkspace(client),
                runtime,
                model_catalogue=catalogue,
            )
            result = agent.run(
                task=args.task,
                domain=args.domain,
                credentials=session,
                model_candidates=candidates,
                capability=args.capability,
                required_model_capabilities=tuple(args.required_model_capability or ()),
                execution_mode=args.execution_mode,
                max_steps=args.max_steps,
                requested_output_tokens=args.requested_output_tokens,
                max_output_tokens=args.requested_output_tokens,
                approve_provider_call=args.approve_provider_call,
                approve_mission_dispatch=args.approve_mission_dispatch,
                run_id=args.run_id,
            )
        return {
            "schema": CLI_SCHEMA,
            "command": "run",
            "result": result,
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

    status = subparsers.add_parser("provider-status", parents=[provider_parent], help="show redacted provider readiness")

    onboarding = subparsers.add_parser("onboard", parents=[provider_parent], help="collect one short-lived key and show redacted status")
    _add_credential_arguments(onboarding)

    run = subparsers.add_parser("run", parents=[provider_parent], help="run one autonomous task through a caller-owned MCP workspace")
    run.add_argument("--mcp-command", required=True, help="MCP executable and arguments; no shell is invoked")
    run.add_argument("--mcp-cwd", default=None, help="working directory for the MCP process")
    run.add_argument("--mcp-timeout", type=float, default=_DEFAULT_TIMEOUT)
    run.add_argument("--task", required=True)
    run.add_argument("--domain", required=True, choices=AUTONOMOUS_DOMAINS)
    run.add_argument("--model", action="append", required=True, help="candidate model; repeat to enable model selection")
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
