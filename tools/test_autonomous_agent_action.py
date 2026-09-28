from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import sys

import pytest

import autonomous_agent_action as action_module
from autonomous_agent_action import (
    AutonomousAgentActionError,
    build_cli_argv,
    main as action_main,
    run_action,
)
from prism_sdk.cli import CLI_SCHEMA


ROOT = Path(__file__).resolve().parents[1]
ACTION_MANIFEST = ROOT / ".github" / "actions" / "autonomous-run" / "action.yml"
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
SETUP_PYTHON_PIN = "actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97"


def _environment(workspace: Path, **values: str) -> dict[str, str]:
    result = {
        "GITHUB_WORKSPACE": str(workspace),
        "GITHUB_RUN_ID": "fixture-run-42",
        "GITHUB_OUTPUT": str(workspace / "github-output.txt"),
        "INPUT_TASK": "private task with shell text: $(touch should-not-run)\nand another line",
        "INPUT_MCP_COMMAND": "python -u fake-mcp-server",
        "INPUT_PROVIDER": "local",
        "INPUT_MODELS": "local-model",
        "INPUT_DOMAIN": "coding",
        "INPUT_EXECUTION_MODE": "provider",
        "INPUT_APPROVE_PROVIDER_CALL": "false",
        "INPUT_APPROVE_MISSION_DISPATCH": "false",
    }
    result.update(values)
    return result


def test_composite_action_manifest_wires_the_reviewed_runner_contract() -> None:
    manifest = ACTION_MANIFEST.read_text(encoding="utf-8")
    workflow = CI_WORKFLOW.read_text(encoding="utf-8")
    release_workflow = RELEASE_WORKFLOW.read_text(encoding="utf-8")

    assert "using: composite" in manifest
    assert SETUP_PYTHON_PIN in manifest
    assert SETUP_PYTHON_PIN in workflow
    assert SETUP_PYTHON_PIN in release_workflow
    assert 'INPUT_APPROVE_PROVIDER_CALL: ${{ inputs[\'approve-provider-call\'] }}' in manifest
    assert 'INPUT_APPROVE_MISSION_DISPATCH: ${{ inputs[\'approve-mission-dispatch\'] }}' in manifest
    assert 'INPUT_ALLOW_MCP_TOOLS: ${{ inputs[\'allow-mcp-tools\'] }}' in manifest
    assert 'run: python "$GITHUB_ACTION_PATH/../../../tools/autonomous_agent_action.py"' in manifest
    assert "value: ${{ steps.run.outputs.result_digest }}" in manifest
    assert "uses: ./.github/actions/autonomous-run" in workflow
    assert "provider: local" in workflow


def test_action_builds_an_in_process_argv_and_keeps_provider_approval_explicit(tmp_path: Path) -> None:
    task = "review '$HOME' and $(echo never-run)\nkeep both lines"
    environment = _environment(tmp_path, INPUT_TASK=task)

    argv = build_cli_argv(environment)

    assert argv[0] == "run"
    assert argv[argv.index("--task") + 1] == task
    assert argv[argv.index("--mcp-command") + 1] == "python -u fake-mcp-server"
    assert argv[argv.index("--run-id") + 1] == "fixture-run-42"
    assert argv[argv.index("--mcp-timeout") + 1] == "30"
    assert "--automatic" not in argv
    assert "--approve-provider-call" not in argv
    assert "--approve-mission-dispatch" not in argv


@pytest.mark.parametrize(
    ("input_name", "value", "bound_name"),
    (
        ("INPUT_MAX_DOMAINS", "0", "max-domains"),
        ("INPUT_MAX_DOMAINS", "5", "max-domains"),
        ("INPUT_MAX_DOMAINS", "99999999999999999999", "max-domains"),
        ("INPUT_MAX_STEPS", "0", "max-steps"),
        ("INPUT_MAX_STEPS", "129", "max-steps"),
        ("INPUT_MAX_STEPS", "9" * 5_000, "max-steps"),
    ),
)
def test_action_enforces_cli_domain_and_step_limits_before_dispatch(
    tmp_path: Path, input_name: str, value: str, bound_name: str
) -> None:
    environment = _environment(tmp_path, **{input_name: value})
    with pytest.raises(AutonomousAgentActionError, match=bound_name):
        build_cli_argv(environment)


def test_action_accepts_exact_domain_and_step_limits(tmp_path: Path) -> None:
    environment = _environment(tmp_path, INPUT_MAX_DOMAINS="4", INPUT_MAX_STEPS="128")
    argv = build_cli_argv(environment)
    assert argv[argv.index("--max-domains") + 1] == "4"
    assert argv[argv.index("--max-steps") + 1] == "128"


def test_action_requires_exact_tool_allowlist_and_independent_effect_approval(tmp_path: Path) -> None:
    tool_loop = _environment(tmp_path, INPUT_EXECUTION_MODE="tool_loop")
    with pytest.raises(AutonomousAgentActionError, match="exact tools"):
        build_cli_argv(tool_loop)

    tool_loop["INPUT_ALLOW_MCP_TOOLS"] = "read_status\nsearch_records"
    tool_loop["INPUT_APPROVE_PROVIDER_CALL"] = "true"
    argv = build_cli_argv(tool_loop)
    assert argv.count("--allow-mcp-tool") == 2
    assert argv[argv.index("--allow-mcp-tool") + 1] == "read_status"
    assert "--approve-provider-call" in argv
    assert "--approve-mission-dispatch" not in argv

    mission = dict(tool_loop, INPUT_EXECUTION_MODE="mission", INPUT_APPROVE_MISSION_DISPATCH="true")
    mission["INPUT_APPROVE_PROVIDER_CALL"] = "false"
    with pytest.raises(AutonomousAgentActionError, match="provider-call approval"):
        build_cli_argv(mission)

    effect_without_tool_mode = _environment(
        tmp_path,
        INPUT_APPROVE_PROVIDER_CALL="true",
        INPUT_APPROVE_MISSION_DISPATCH="true",
    )
    with pytest.raises(AutonomousAgentActionError, match="tool_loop or mission"):
        build_cli_argv(effect_without_tool_mode)


def test_action_rejects_invalid_credential_names_and_result_paths_before_execution(tmp_path: Path) -> None:
    invalid_credential = _environment(tmp_path, INPUT_PROVIDER="openai", INPUT_CREDENTIAL_ENV="API_KEY; echo leak")
    with pytest.raises(AutonomousAgentActionError, match="environment variable name"):
        build_cli_argv(invalid_credential)

    invoked = False

    def should_not_run(*_args, **_kwargs) -> int:
        nonlocal invoked
        invoked = True
        return 0

    escaping_path = _environment(tmp_path, INPUT_RESULT_FILE="../outside.json")
    with pytest.raises(AutonomousAgentActionError, match="inside GITHUB_WORKSPACE"):
        run_action(escaping_path, command=should_not_run)
    assert not invoked

    invalid_timeout = _environment(tmp_path, INPUT_MCP_TIMEOUT="nan")
    with pytest.raises(AutonomousAgentActionError, match="mcp-timeout"):
        build_cli_argv(invalid_timeout)

    multiline_path = _environment(tmp_path, INPUT_RESULT_FILE="result.json\nstatus=completed")
    with pytest.raises(AutonomousAgentActionError, match="single-line path"):
        run_action(multiline_path, command=should_not_run)
    assert not invoked


def test_action_hides_inputs_and_provider_key_from_mcp_child_environment(tmp_path: Path, monkeypatch) -> None:
    environment = _environment(tmp_path, INPUT_PROVIDER="openai", INPUT_CREDENTIAL_ENV="AURORA_ACTION_TEST_KEY")
    environment["AURORA_ACTION_TEST_KEY"] = "provider-secret-fixture"
    environment["INPUT_TASK"] = "private task fixture"
    github_output = tmp_path / "github-output.txt"
    monkeypatch.setenv("INPUT_TASK", environment["INPUT_TASK"])
    monkeypatch.setenv("INPUT_MCP_COMMAND", environment["INPUT_MCP_COMMAND"])
    monkeypatch.setenv("INPUT_PROVIDER", "openai")
    monkeypatch.setenv("INPUT_MODELS", "fixture-model")
    monkeypatch.setenv("INPUT_DOMAIN", "coding")
    monkeypatch.setenv("INPUT_CREDENTIAL_ENV", "AURORA_ACTION_TEST_KEY")
    monkeypatch.setenv("AURORA_ACTION_TEST_KEY", "provider-secret-fixture")
    monkeypatch.setenv("GITHUB_RUN_ID", "fixture-run-42")
    monkeypatch.setenv("GITHUB_WORKSPACE", str(tmp_path))
    monkeypatch.setenv("GITHUB_OUTPUT", str(github_output))
    monkeypatch.setenv("INPUT_UNRELATED_AMBIENT", "ambient-private-input")
    monkeypatch.delenv("INPUT_APPROVE_PROVIDER_CALL", raising=False)
    monkeypatch.delenv("INPUT_APPROVE_MISSION_DISPATCH", raising=False)
    captured: dict[str, object] = {}

    def fake_cli(argv, *, environ, writer, error_writer) -> int:
        captured["task_in_cli_environment"] = environ["INPUT_TASK"]
        captured["credential_in_cli_environment"] = environ["AURORA_ACTION_TEST_KEY"]
        captured["task_in_child_environment"] = "INPUT_TASK" in os.environ
        captured["ambient_input_in_child_environment"] = "INPUT_UNRELATED_AMBIENT" in os.environ
        captured["credential_in_child_environment"] = "AURORA_ACTION_TEST_KEY" in os.environ
        writer.write(json.dumps({"schema": CLI_SCHEMA, "command": "run", "routing_mode": "explicit_domain", "result": {"status": "completed"}}))
        return 0

    run_action(environment, command=fake_cli)
    assert captured == {
        "task_in_cli_environment": "private task fixture",
        "credential_in_cli_environment": "provider-secret-fixture",
        "task_in_child_environment": False,
        "ambient_input_in_child_environment": False,
        "credential_in_child_environment": False,
    }
    assert os.environ["INPUT_TASK"] == "private task fixture"
    assert os.environ["INPUT_UNRELATED_AMBIENT"] == "ambient-private-input"
    assert os.environ["AURORA_ACTION_TEST_KEY"] == "provider-secret-fixture"


def test_action_outputs_only_status_and_digest_unless_full_result_is_requested(tmp_path: Path) -> None:
    environment = _environment(tmp_path)
    private_task = environment["INPUT_TASK"]
    private_model_text = "sensitive model response that must not reach outputs"
    payload = {
        "schema": CLI_SCHEMA,
        "command": "run",
        "routing_mode": "explicit_domain",
        "result": {"status": "completed", "answer": private_model_text},
    }
    captured: dict[str, object] = {}

    def fake_cli(argv, *, environ, writer, error_writer) -> int:
        captured["argv"] = list(argv)
        captured["environ"] = environ
        writer.write(json.dumps(payload, sort_keys=True))
        return 0

    outputs = run_action(environment, command=fake_cli)
    output_text = Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
    assert outputs["status"] == "completed"
    assert outputs["routing_mode"] == "explicit_domain"
    assert outputs["run_id"] == "fixture-run-42"
    assert outputs["result_digest"] == hashlib.sha256(
        json.dumps(payload, sort_keys=True).encode("utf-8")
    ).hexdigest()
    assert private_task not in output_text
    assert private_model_text not in output_text
    assert "task" not in output_text.lower()
    assert captured["argv"][captured["argv"].index("--task") + 1] == private_task

    result_file = "reports/agent-result.json"
    environment["INPUT_RESULT_FILE"] = result_file
    Path(environment["GITHUB_OUTPUT"]).write_text("", encoding="utf-8")
    outputs = run_action(environment, command=fake_cli)
    assert outputs["result_file"] == str(tmp_path / result_file)
    persisted = Path(outputs["result_file"]).read_text(encoding="utf-8")
    assert private_model_text in persisted
    assert private_task not in output_text  # the opt-in file is separate from GitHub outputs


@pytest.mark.parametrize(
    ("routing_mode", "status"),
    (("explicit_domain", None), ("unexpected", "completed"), ([], "completed")),
)
def test_action_rejects_malformed_success_projection_before_writing_outputs(
    tmp_path: Path, routing_mode: object, status: str | None
) -> None:
    environment = _environment(tmp_path)
    environment["INPUT_RESULT_FILE"] = "result.json"
    payload = {
        "schema": CLI_SCHEMA,
        "command": "run",
        "routing_mode": routing_mode,
        "result": {"status": status},
    }

    def malformed_cli(_argv, *, environ, writer, error_writer) -> int:
        writer.write(json.dumps(payload))
        return 0

    with pytest.raises(AutonomousAgentActionError, match="status|routing mode"):
        run_action(environment, command=malformed_cli)
    assert not Path(environment["GITHUB_OUTPUT"]).exists()
    assert not (tmp_path / "result.json").exists()


def test_action_enforces_result_byte_bound_while_cli_writes(
    tmp_path: Path, monkeypatch
) -> None:
    environment = _environment(tmp_path)
    monkeypatch.setattr(action_module, "MAX_ACTION_RESULT_BYTES", 8)

    def oversized_cli(_argv, *, environ, writer, error_writer) -> int:
        writer.write("123456789")
        return 0

    with pytest.raises(AutonomousAgentActionError, match="exceeds its byte bound"):
        run_action(environment, command=oversized_cli)
    assert not Path(environment["GITHUB_OUTPUT"]).exists()


def test_action_runs_local_provider_and_caller_owned_mcp_end_to_end(tmp_path: Path, monkeypatch) -> None:
    fixture = ROOT / "python" / "tests" / "autonomous_brain_mcp_server.py"
    command = f'"{sys.executable.replace(chr(92), "/")}" -u "{fixture.as_posix()}"'
    environment = _environment(
        tmp_path,
        INPUT_TASK="bounded local action integration task",
        INPUT_MCP_COMMAND=command,
        INPUT_PROVIDER="local",
        INPUT_MODELS="local-model",
        INPUT_DOMAIN="coding",
        INPUT_APPROVE_PROVIDER_CALL="true",
        INPUT_MCP_WORKING_DIRECTORY=str(ROOT),
    )
    environment["INPUT_CREDENTIAL_ENV"] = "OPENAI_API_KEY"
    for name, value in environment.items():
        monkeypatch.setenv(name, value)

    output = io.StringIO()
    exit_code = action_main(output=output)
    assert exit_code == 0, output.getvalue()

    output_text = Path(environment["GITHUB_OUTPUT"]).read_text(encoding="utf-8")
    outputs = dict(line.split("=", 1) for line in output_text.splitlines())
    assert outputs["status"] == "completed_provider_call"
    assert outputs["routing_mode"] == "explicit_domain"
    assert outputs["run_id"] == "fixture-run-42"
    assert len(outputs["result_digest"]) == 64
    assert "bounded local action integration task" not in output_text
    assert "Local provider completed the requested task." not in output_text
    assert "bounded local action integration task" not in output.getvalue()
