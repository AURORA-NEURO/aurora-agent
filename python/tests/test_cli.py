from __future__ import annotations

import io
import json
from unittest.mock import patch

from prism_sdk import AUTONOMOUS_DOMAINS, ProviderModelDescriptor
from prism_sdk.cli import main


def _invoke(*args: str, environ: dict[str, str] | None = None, reader=None):
    output = io.StringIO()
    errors = io.StringIO()
    code = main(
        args,
        environ={} if environ is None else environ,
        reader=reader,
        writer=output,
        error_writer=errors,
    )
    return code, json.loads(output.getvalue()) if output.getvalue() else None, errors.getvalue()


def test_catalogue_and_evidence_plan_cover_every_autonomous_domain_without_credentials() -> None:
    code, payload, errors = _invoke("catalogue")
    assert code == 0
    assert errors == ""
    assert len(payload["domains"]) == 12
    assert len(payload["domain_packs"]) == 12
    assert len(payload["evaluators"]) == 12
    assert payload["secret_material"] == "never_returned"

    code, plan, errors = _invoke("evidence-plan")
    assert code == 0
    assert errors == ""
    assert set(plan["domains"]) == set(AUTONOMOUS_DOMAINS)
    assert plan["coverage_status"] == "not_evaluated"


def test_route_is_provider_free_and_secret_safe() -> None:
    code, payload, errors = _invoke("route", "--task", "compare two research hypotheses")
    assert code == 0
    assert errors == ""
    assert payload["route"]["selected_domains"]
    assert payload["authorization"] == "routing_evidence_only; no_tools_or_effects_authorized"


def test_provider_status_never_collects_or_returns_a_key() -> None:
    code, payload, errors = _invoke(
        "provider-status",
        "--provider", "openai",
        "--base-url", "https://provider.example",
    )
    assert code == 0
    assert errors == ""
    encoded = json.dumps(payload)
    assert payload["status"]["ready"] is False
    assert payload["instructions"]["next_action"] == "collect_user_credential"
    assert "api_key" not in encoded
    assert "secret_material" in encoded


def test_onboard_uses_no_echo_reader_and_closes_the_credential_session() -> None:
    secret = "cli-test-secret-that-must-not-appear"
    prompts: list[str] = []

    def reader(prompt: str) -> str:
        prompts.append(prompt)
        return secret

    code, payload, errors = _invoke(
        "onboard",
        "--provider", "openai",
        "--base-url", "https://provider.example",
        reader=reader,
    )
    assert code == 0
    assert errors == ""
    assert prompts == ["openai API key (input hidden): "]
    assert payload["session_closed"] is True
    assert payload["session"]["active"] is False
    assert payload["session"]["secret_material"] == "never_returned"
    assert secret not in json.dumps(payload)


def test_environment_onboarding_reports_only_the_variable_name() -> None:
    secret = "environment-cli-secret"
    code, payload, errors = _invoke(
        "onboard",
        "--provider", "openai",
        "--base-url", "https://provider.example",
        "--credential-source", "environment",
        "--credential-env", "AURORA_TEST_KEY",
        environ={"AURORA_TEST_KEY": secret},
    )
    assert code == 0
    assert errors == ""
    assert payload["provider"]["credential"]["credentials"][0]["source"] == "environment"
    assert secret not in json.dumps(payload)


def test_discover_models_requires_explicit_provider_approval() -> None:
    code, payload, errors = _invoke(
        "discover-models",
        "--provider", "openai",
        "--base-url", "https://provider.example",
        "--credential-source", "environment",
        "--credential-env", "AURORA_TEST_KEY",
        environ={"AURORA_TEST_KEY": "discovery-gate-secret"},
    )
    assert code == 2
    assert payload is None
    assert "command failed" in errors
    assert "discovery-gate-secret" not in errors


def test_refresh_models_requires_explicit_provider_approval() -> None:
    code, payload, errors = _invoke(
        "refresh-models",
        "--provider", "openai",
        "--base-url", "https://provider.example",
        "--credential-source", "environment",
        "--credential-env", "AURORA_TEST_KEY",
        environ={"AURORA_TEST_KEY": "refresh-gate-secret"},
    )
    assert code == 2
    assert payload is None
    assert "command failed" in errors
    assert "refresh-gate-secret" not in errors


def test_refresh_models_passes_typed_prior_factory_and_closes_credentials(tmp_path) -> None:
    captured: dict[str, object] = {}
    secret = "refresh-test-secret-that-must-not-appear"

    def fake_refresh(self, **kwargs: object) -> dict[str, object]:
        captured.update(kwargs)
        return {
            "status": "completed",
            "snapshot_digest": "a" * 64,
            "providers": [{"provider": "openai", "status": "refreshed"}],
            "coverage": [],
        }

    with patch("prism_sdk.cli.AutonomousAgent.refresh_model_inventory", fake_refresh):
        code, payload, errors = _invoke(
            "refresh-models",
            "--provider", "openai",
            "--base-url", "https://provider.example",
            "--credential-source", "environment",
            "--credential-env", "AURORA_TEST_KEY",
            "--model-capability", "reasoning",
            "--inventory-store", str(tmp_path / "inventory.json"),
            "--approve-provider-call",
            environ={"AURORA_TEST_KEY": secret},
        )
    assert code == 0
    assert errors == ""
    assert payload["command"] == "refresh-models"
    assert payload["inventory_store"]["persisted"] is True
    assert payload["credential_session"]["active"] is False
    assert payload["authorization"]["model_inventory_refresh_approved"] is True
    prior_factory = captured["prior_factory"]
    descriptor = ProviderModelDescriptor(
        provider="openai",
        model="factory-model",
        context_window_tokens=8_192,
        max_output_tokens=1_024,
        metadata={"owned_by": "test"},
    )
    prior = prior_factory(descriptor)
    assert prior["quality"] == 0.5
    assert prior["capabilities"] == ("reasoning",)
    assert prior["context_window_tokens"] == 8_192
    assert secret not in json.dumps(payload)


def test_inventory_status_is_metadata_only_and_provider_free(tmp_path) -> None:
    code, payload, errors = _invoke(
        "inventory-status",
        "--inventory-store", str(tmp_path / "missing-inventory.json"),
    )
    assert code == 0
    assert errors == ""
    assert payload["available"] is False
    assert payload["authorization"] == "metadata_read_only; no_provider_or_credential_access"
    assert payload["secret_material"] == "never_returned"


def test_discover_models_projects_only_typed_metadata_and_closes_credentials() -> None:
    secret = "discovery-test-secret-that-must-not-appear"
    descriptors = (
        ProviderModelDescriptor(
            provider="openai",
            model="model-a",
            capabilities=("tool_calling",),
            context_window_tokens=16_384,
            max_output_tokens=2_048,
            metadata={"owned_by": "test-provider", "created": 123},
        ),
    )
    with patch("prism_sdk.cli.LLMRuntime.discover_models", return_value=descriptors):
        code, payload, errors = _invoke(
            "discover-models",
            "--provider", "openai",
            "--base-url", "https://provider.example",
            "--credential-source", "environment",
            "--credential-env", "AURORA_TEST_KEY",
            "--approve-provider-call",
            environ={"AURORA_TEST_KEY": secret},
        )
    assert code == 0
    assert errors == ""
    assert payload["model_count"] == 1
    assert payload["models"][0]["model"] == "model-a"
    assert payload["models"][0]["context_window_tokens"] == 16_384
    assert payload["models"][0]["credential_posture"] == "caller_supplied_opaque_handle_not_returned"
    assert payload["credential_session"]["active"] is False
    assert payload["authorization"]["model_discovery_approved"] is True
    assert secret not in json.dumps(payload)


def test_cli_rejects_invalid_commands_without_echoing_argument_text() -> None:
    secret = "unknown-argument-secret"
    code, payload, errors = _invoke("provider-status", "--api-key", secret)
    assert code != 0
    assert payload is None
    assert secret not in errors


def test_run_requires_explicit_or_automatic_routing_mode() -> None:
    code, payload, errors = _invoke(
        "run",
        "--mcp-command", "python server.py",
        "--task", "inspect the repository",
        "--model", "offline-model",
        "--credential-source", "environment",
        "--credential-env", "AURORA_TEST_KEY",
        environ={"AURORA_TEST_KEY": "routing-test-value"},
    )
    assert code == 2
    assert payload is None
    assert "command failed" in errors


def test_run_automatic_mode_forwards_routing_and_planning_controls_without_provider_payloads() -> None:
    captured: dict[str, object] = {}

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, *, model_catalogue: object) -> None:
            captured["catalogue"] = model_catalogue

        def run_auto(self, **kwargs: object) -> dict[str, object]:
            captured.update(kwargs)
            return {"status": "completed", "route": {"selected_domains": ["research"]}}

    secret = "cli-automatic-test-value"
    output = io.StringIO()
    errors = io.StringIO()
    with patch("prism_sdk.cli.AutonomousAgent", FakeAgent):
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--automatic",
                "--task", "compare independent research sources",
                "--hint", "research",
                "--model", "model-a",
                "--model", "model-b",
                "--provider", "openai",
                "--base-url", "https://provider.example",
                "--credential-source", "environment",
                "--credential-env", "AURORA_TEST_KEY",
                "--planning-mode", "provider",
                "--semantic-routing",
                "--approve-provider-call",
            ),
            environ={"AURORA_TEST_KEY": secret},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    payload = json.loads(output.getvalue())
    assert code == 0
    assert errors.getvalue() == ""
    assert captured["hints"] == ("research",)
    assert captured["planning_mode"] == "provider"
    assert captured["semantic_routing"] is True
    assert captured["allow_cross_domain"] is True
    assert len(captured["model_candidates"]) == 2
    assert payload["routing_mode"] == "automatic"
    assert secret not in output.getvalue()


def test_run_can_build_candidates_from_discovery_and_filter_archived_models() -> None:
    captured: dict[str, object] = {}

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, *, model_catalogue: object) -> None:
            captured["catalogue"] = model_catalogue

        def run(self, **kwargs: object) -> dict[str, object]:
            captured.update(kwargs)
            return {"status": "completed", "model": kwargs["model_candidates"][0].model}

    descriptors = (
        ProviderModelDescriptor(
            provider="openai",
            model="model-a",
            capabilities=("tool_calling",),
            context_window_tokens=8_192,
            max_output_tokens=1_024,
            metadata={"owned_by": "test-provider"},
        ),
        ProviderModelDescriptor(
            provider="openai",
            model="model-archived",
            context_window_tokens=8_192,
            max_output_tokens=1_024,
            metadata={"archived": True},
        ),
    )
    secret = "discovery-run-secret-that-must-not-appear"
    output = io.StringIO()
    errors = io.StringIO()
    with (
        patch("prism_sdk.cli.LLMRuntime.discover_models", return_value=descriptors),
        patch("prism_sdk.cli.AutonomousAgent", FakeAgent),
    ):
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--domain", "science",
                "--task", "compare independent research sources",
                "--discover-models",
                "--provider", "openai",
                "--base-url", "https://provider.example",
                "--credential-source", "environment",
                "--credential-env", "AURORA_TEST_KEY",
                "--approve-provider-call",
            ),
            environ={"AURORA_TEST_KEY": secret},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    payload = json.loads(output.getvalue())
    assert code == 0
    assert errors.getvalue() == ""
    candidates = captured["model_candidates"]
    assert [candidate.model for candidate in candidates] == ["model-a"]
    assert candidates[0].context_window_tokens == 8_192
    assert candidates[0].max_output_tokens == 1_024
    assert candidates[0].capabilities == ("tool_calling",)
    assert payload["model_inventory"]["mode"] == "provider_discovery"
    assert payload["model_inventory"]["model_count"] == 2
    assert payload["authorization"]["model_discovery_approved"] is True
    assert secret not in output.getvalue()
