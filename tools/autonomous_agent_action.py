"""GitHub Actions entry point for the caller-owned Aurora autonomous CLI."""

from __future__ import annotations

import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import sys
import tempfile
import uuid
from collections.abc import Callable, Mapping
from typing import TextIO


ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from prism_sdk.cli import CLI_SCHEMA, main as cli_main  # noqa: E402
from prism_sdk import MAX_AUTONOMOUS_ROUTE_DOMAINS, MAX_AUTONOMOUS_TASK_STEPS  # noqa: E402


MAX_ACTION_TASK_BYTES = 32_000
MAX_ACTION_MCP_COMMAND_BYTES = 16_384
MAX_ACTION_RESULT_BYTES = 32_000_000
_ENVIRONMENT_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]{0,127}\Z")
_RUN_ID = re.compile(r"[A-Za-z0-9_.:-]{1,128}\Z")
_STATUS = re.compile(r"[a-z][a-z0-9_]{0,63}\Z")


class AutonomousAgentActionError(ValueError):
    """A caller input, CLI response, or output path violates the action contract."""


class _BoundedTextBuffer(io.TextIOBase):
    """Capture CLI output without retaining more than the action's byte limit."""

    def __init__(self, maximum_bytes: int) -> None:
        self._maximum_bytes = maximum_bytes
        self._buffer = io.StringIO()
        self._size_bytes = 0

    def write(self, value: str) -> int:
        if not isinstance(value, str):
            raise TypeError("CLI output must be text")
        value_size = len(value) if value.isascii() else len(value.encode("utf-8"))
        if self._size_bytes + value_size > self._maximum_bytes:
            raise AutonomousAgentActionError("autonomous CLI result exceeds its byte bound")
        self._buffer.write(value)
        self._size_bytes += value_size
        return len(value)

    def getvalue(self) -> str:
        return self._buffer.getvalue()


class _DiscardTextWriter(io.TextIOBase):
    """Accept diagnostics that must not be retained or exposed by the action."""

    def write(self, value: str) -> int:
        if not isinstance(value, str):
            raise TypeError("CLI diagnostics must be text")
        return len(value)


def _input(environ: Mapping[str, str], name: str, default: str = "") -> str:
    value = environ.get(f"INPUT_{name.upper().replace('-', '_')}", default)
    return value if isinstance(value, str) else default


def _required(environ: Mapping[str, str], name: str) -> str:
    value = _input(environ, name)
    if not value.strip():
        raise AutonomousAgentActionError(f"{name} is required")
    return value


def _boolean(environ: Mapping[str, str], name: str, *, default: bool = False) -> bool:
    raw = _input(environ, name, "true" if default else "false").strip().lower()
    if raw in {"true", "1", "yes"}:
        return True
    if raw in {"false", "0", "no"}:
        return False
    raise AutonomousAgentActionError(f"{name} must be true or false")


def _lines(environ: Mapping[str, str], name: str, *, maximum: int, item_limit: int = 256) -> tuple[str, ...]:
    raw = _input(environ, name)
    if len(raw) > maximum * (item_limit + 2):
        raise AutonomousAgentActionError(f"{name} exceeds its bounded list contract")
    values: list[str] = []
    for line in io.StringIO(raw, newline=None):
        value = line.strip()
        if not value:
            continue
        if len(value) > item_limit or len(value.encode("utf-8")) > item_limit:
            raise AutonomousAgentActionError(f"{name} exceeds its bounded list contract")
        values.append(value)
        if len(values) > maximum:
            raise AutonomousAgentActionError(f"{name} exceeds its bounded list contract")
    if len(set(values)) != len(values):
        raise AutonomousAgentActionError(f"{name} entries must be unique")
    return values


def _bounded_positive_integer(environ: Mapping[str, str], name: str, *, default: str, maximum: int) -> int:
    raw = _input(environ, name, default).strip()
    if not raw.isascii() or not raw.isdecimal() or len(raw) > len(str(maximum)):
        raise AutonomousAgentActionError(f"{name} must be between 1 and {maximum}")
    value = int(raw)
    if not 1 <= value <= maximum:
        raise AutonomousAgentActionError(f"{name} must be between 1 and {maximum}")
    return value


def build_cli_argv(environ: Mapping[str, str]) -> list[str]:
    """Build an in-process CLI argument vector; caller text never enters a shell command."""

    task = _input(environ, "task")
    if len(task) > MAX_ACTION_TASK_BYTES:
        raise AutonomousAgentActionError("task exceeds the bounded UTF-8 input contract")
    if not task.strip():
        raise AutonomousAgentActionError("task is required")
    if len(task.encode("utf-8")) > MAX_ACTION_TASK_BYTES or "\x00" in task:
        raise AutonomousAgentActionError("task exceeds the bounded UTF-8 input contract")
    mcp_command = _input(environ, "mcp-command")
    if (
        len(mcp_command) > MAX_ACTION_MCP_COMMAND_BYTES
        or len(mcp_command.encode("utf-8")) > MAX_ACTION_MCP_COMMAND_BYTES
        or "\x00" in mcp_command
    ):
        raise AutonomousAgentActionError("mcp-command exceeds its bounded command contract")
    if not mcp_command.strip():
        raise AutonomousAgentActionError("mcp-command is required")
    provider = _input(environ, "provider", "openai").strip()
    if not provider or len(provider.encode("utf-8")) > 128:
        raise AutonomousAgentActionError("provider is outside its bounded identifier contract")
    models = _lines(environ, "models", maximum=16, item_limit=256)
    if not models:
        raise AutonomousAgentActionError("at least one model is required")
    credential_env = _input(environ, "credential-env", "OPENAI_API_KEY").strip()
    if credential_env and not _ENVIRONMENT_NAME.fullmatch(credential_env):
        raise AutonomousAgentActionError("credential-env must be an environment variable name")
    if provider not in {"local", "in_memory", "ollama"} and not credential_env:
        raise AutonomousAgentActionError("credential-env is required for credentialed providers")

    mode = _input(environ, "execution-mode", "provider").strip()
    if mode not in {"provider", "tool_loop", "mission"}:
        raise AutonomousAgentActionError("execution-mode is unsupported")
    allowed_tools = _lines(environ, "allow-mcp-tools", maximum=128)
    if mode in {"tool_loop", "mission"} and not allowed_tools:
        raise AutonomousAgentActionError("allow-mcp-tools must name the exact tools exposed to the provider")
    if mode == "provider" and allowed_tools:
        raise AutonomousAgentActionError("allow-mcp-tools requires tool_loop or mission execution mode")
    approve_provider = _boolean(environ, "approve-provider-call")
    approve_mission = _boolean(environ, "approve-mission-dispatch")
    if approve_mission and not approve_provider:
        raise AutonomousAgentActionError("mission dispatch approval requires provider-call approval")
    if approve_mission and mode not in {"tool_loop", "mission"}:
        raise AutonomousAgentActionError("mission dispatch approval requires tool_loop or mission execution mode")

    argv = [
        "run",
        "--mcp-command",
        mcp_command,
        "--task",
        task,
        "--provider",
        provider,
        "--credential-source",
        "environment",
        "--execution-mode",
        mode,
    ]
    mcp_timeout = _input(environ, "mcp-timeout", "30").strip()
    try:
        timeout_value = float(mcp_timeout)
    except ValueError as error:
        raise AutonomousAgentActionError("mcp-timeout must be between 1 and 600 seconds") from error
    if not math.isfinite(timeout_value) or not 1 <= timeout_value <= 600:
        raise AutonomousAgentActionError("mcp-timeout must be between 1 and 600 seconds")
    argv.extend(("--mcp-timeout", mcp_timeout))
    for model in models:
        argv.extend(("--model", model))
    if credential_env:
        argv.extend(("--credential-env", credential_env))
    mcp_cwd = _input(environ, "mcp-working-directory").strip()
    if mcp_cwd:
        argv.extend(("--mcp-cwd", mcp_cwd))
    domain = _input(environ, "domain").strip()
    if domain:
        argv.extend(("--domain", domain))
    else:
        argv.append("--automatic")
    for hint in _lines(environ, "hints", maximum=32, item_limit=512):
        argv.extend(("--hint", hint))
    for tool_name in allowed_tools:
        argv.extend(("--allow-mcp-tool", tool_name))
    for approved, cli_flag in (
        (approve_provider, "--approve-provider-call"),
        (approve_mission, "--approve-mission-dispatch"),
        (_boolean(environ, "single-domain"), "--single-domain"),
    ):
        if approved:
            argv.append(cli_flag)
    run_id = _input(environ, "run-id").strip() or environ.get("GITHUB_RUN_ID", "").strip()
    if run_id:
        if not _RUN_ID.fullmatch(run_id):
            raise AutonomousAgentActionError("run-id is outside its bounded identifier contract")
        argv.extend(("--run-id", run_id))
    max_domains = _bounded_positive_integer(
        environ, "max-domains", default="3", maximum=MAX_AUTONOMOUS_ROUTE_DOMAINS
    )
    max_steps = _bounded_positive_integer(
        environ, "max-steps", default="8", maximum=MAX_AUTONOMOUS_TASK_STEPS
    )
    argv.extend(("--max-domains", str(max_domains), "--max-steps", str(max_steps)))
    base_url = _input(environ, "base-url").strip()
    if base_url:
        argv.extend(("--base-url", base_url))
    return argv


def _result_path(environ: Mapping[str, str]) -> Path | None:
    raw = _input(environ, "result-file").strip()
    if not raw:
        return None
    workspace = environ.get("GITHUB_WORKSPACE", "").strip()
    if not workspace:
        raise AutonomousAgentActionError("GITHUB_WORKSPACE is required when result-file is set")
    if any(character in raw for character in "\r\n\x00"):
        raise AutonomousAgentActionError("result-file must be a single-line path")
    root = Path(workspace).resolve()
    requested = Path(raw)
    target = requested if requested.is_absolute() else root / requested
    resolved_parent = target.parent.resolve()
    final_path = resolved_parent / target.name
    try:
        final_path.relative_to(root)
    except ValueError as error:
        raise AutonomousAgentActionError("result-file must remain inside GITHUB_WORKSPACE") from error
    if final_path.name in {"", ".", ".."}:
        raise AutonomousAgentActionError("result-file must name a file")
    return final_path


def _write_result_file(path: Path, payload_text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            newline="\n",
            prefix=f".{path.name}.",
            suffix=".tmp",
            dir=path.parent,
            delete=False,
        ) as temporary:
            temporary_name = temporary.name
            temporary.write(payload_text)
        os.replace(temporary_name, path)
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass


def _write_github_output(path: str, values: Mapping[str, str]) -> None:
    if not path:
        return
    with Path(path).open("a", encoding="utf-8", newline="") as output:
        for name, value in values.items():
            if "\n" in value or "\r" in value or "\x00" in value:
                raise AutonomousAgentActionError("GitHub output value is outside its single-line contract")
            output.write(f"{name}={value}\n")


def _call_cli_with_private_environment(
    command: Callable[..., int],
    argv: list[str],
    *,
    environ: Mapping[str, str],
    writer: TextIO,
    error_writer: TextIO,
) -> int:
    """Keep action inputs and the provider key out of the spawned MCP process environment."""

    cli_environ = dict(environ)
    credential_names = {
        _input(source, "credential-env", "OPENAI_API_KEY").strip()
        for source in (environ, os.environ)
    }
    private_names = {
        name
        for source in (environ, os.environ)
        for name in source
        if name.startswith("INPUT_")
    }
    private_names.update(name for name in credential_names if name)
    removed = {name: os.environ.pop(name) for name in private_names if name in os.environ}
    try:
        return command(argv, environ=cli_environ, writer=writer, error_writer=error_writer)
    finally:
        os.environ.update(removed)


def run_action(
    environ: Mapping[str, str],
    *,
    command: Callable[..., int] = cli_main,
) -> dict[str, str]:
    """Run one task without logging or returning the raw CLI payload to workflow logs."""

    run_id = _input(environ, "run-id").strip() or environ.get("GITHUB_RUN_ID", "").strip()
    if not run_id:
        run_id = uuid.uuid4().hex
    if not _RUN_ID.fullmatch(run_id):
        raise AutonomousAgentActionError("run-id is outside its bounded identifier contract")
    effective_environ = dict(environ)
    effective_environ["INPUT_RUN_ID"] = run_id
    argv = build_cli_argv(effective_environ)
    result_path = _result_path(environ)
    output = _BoundedTextBuffer(MAX_ACTION_RESULT_BYTES)
    errors = _DiscardTextWriter()
    exit_code = _call_cli_with_private_environment(
        command,
        argv,
        environ=environ,
        writer=output,
        error_writer=errors,
    )
    if exit_code != 0:
        raise AutonomousAgentActionError("autonomous CLI returned a non-zero exit code")
    payload_text = output.getvalue()
    payload_bytes = payload_text.encode("utf-8")
    if not payload_text or len(payload_bytes) > MAX_ACTION_RESULT_BYTES:
        raise AutonomousAgentActionError("autonomous CLI result is empty or exceeds its byte bound")
    try:
        payload = json.loads(payload_text)
    except json.JSONDecodeError as error:
        raise AutonomousAgentActionError("autonomous CLI returned invalid JSON") from error
    if not isinstance(payload, Mapping) or payload.get("schema") != CLI_SCHEMA or payload.get("command") != "run":
        raise AutonomousAgentActionError("autonomous CLI returned an unsupported response")
    result = payload.get("result")
    if not isinstance(result, Mapping):
        raise AutonomousAgentActionError("autonomous CLI result projection is malformed")
    raw_status = result.get("status")
    if not isinstance(raw_status, str) or not _STATUS.fullmatch(raw_status):
        raise AutonomousAgentActionError("autonomous CLI result status is missing or malformed")
    status = raw_status
    digest = hashlib.sha256(payload_bytes).hexdigest()
    routing_mode = payload.get("routing_mode")
    if not isinstance(routing_mode, str) or routing_mode not in {"automatic", "explicit_domain"}:
        raise AutonomousAgentActionError("autonomous CLI routing mode is unsupported")
    if result_path is not None:
        _write_result_file(result_path, payload_text)
    outputs = {
        "status": status,
        "routing_mode": routing_mode,
        "run_id": run_id,
        "result_digest": digest,
        "result_file": "" if result_path is None else str(result_path),
    }
    _write_github_output(environ.get("GITHUB_OUTPUT", "").strip(), outputs)
    return outputs


def main(
    environ: Mapping[str, str] | None = None,
    *,
    output: TextIO | None = None,
    command: Callable[..., int] = cli_main,
) -> int:
    env = os.environ if environ is None else environ
    writer = output or sys.stdout
    try:
        values = run_action(env, command=command)
    except Exception as error:
        # CLI diagnostics are deliberately suppressed here; workflow logs must not contain task,
        # model, MCP, or credential-shaped data even if a future adapter leaks it in an error.
        writer.write(f"aurora-agent-action: {type(error).__name__}: run failed\n")
        return 2
    writer.write(
        f"aurora-agent-action: status={values['status']} run_id={values['run_id']} "
        f"result_digest={values['result_digest']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
