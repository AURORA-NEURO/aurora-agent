from __future__ import annotations

import io
import json

from prism_sdk import AUTONOMOUS_DOMAINS
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


def test_cli_rejects_invalid_commands_without_echoing_argument_text() -> None:
    secret = "unknown-argument-secret"
    code, payload, errors = _invoke("provider-status", "--api-key", secret)
    assert code != 0
    assert payload is None
    assert secret not in errors
