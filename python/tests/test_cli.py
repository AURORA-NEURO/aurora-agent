from __future__ import annotations

import io
import json
from unittest.mock import patch

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    BRAIN_LEARNING_EPISODE_SCHEMA,
    ModelCandidate,
    ModelCatalogue,
    ProviderModelDescriptor,
    SQLiteBrainLearningLedger,
)
from prism_sdk.cli import main


def _invoke(*args: str, environ: dict[str, str] | None = None, reader=None, client_factory=None):
    output = io.StringIO()
    errors = io.StringIO()
    code = main(
        args,
        environ={} if environ is None else environ,
        reader=reader,
        writer=output,
        error_writer=errors,
        **({} if client_factory is None else {"client_factory": client_factory}),
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


def test_state_status_is_provider_free_and_does_not_create_missing_ledgers(tmp_path) -> None:
    health_path = tmp_path / "health.jsonl"
    learning_path = tmp_path / "learning.sqlite"
    code, payload, errors = _invoke(
        "state-status",
        "--health-store", str(health_path),
        "--learning-store", str(learning_path),
    )
    assert code == 0
    assert errors == ""
    assert payload["health"]["available"] is False
    assert payload["learning"]["available"] is False
    assert payload["authorization"] == "metadata_read_only; no_provider_or_credential_access"
    assert not health_path.exists()
    assert not learning_path.exists()


def _write_cli_learning_episode(path, *, episode_id: str = "cli-episode-1", evidence_digest=None) -> None:
    evaluation_input = {
        "schema": "bioprism-brain-evaluator-input/0.1",
        "run_id": "cli-run-1",
        "result_kind": "run",
        "selected_model": {"provider": "offline", "model": "test-model"},
        "selection_digest": "a" * 64,
        "prompt_digest": "b" * 64,
        "plan_digest": "c" * 64,
        "outcome_digest": "d" * 64,
        "evidence_digest": evidence_digest,
    }
    episode = {
        "schema": BRAIN_LEARNING_EPISODE_SCHEMA,
        "episode_id": episode_id,
        "evaluation_input": evaluation_input,
        "arm_id": "offline/test-model",
        "evidence_digest": evidence_digest,
        "status": "pending",
    }
    with SQLiteBrainLearningLedger(path) as ledger:
        ledger.begin_episode(episode)


def test_learning_status_is_provider_free_and_projects_only_episode_digests(tmp_path) -> None:
    path = tmp_path / "learning.sqlite3"
    _write_cli_learning_episode(path)
    code, payload, errors = _invoke(
        "learning-status",
        "--learning-store", str(path),
        "--episode-id", "cli-episode-1",
    )
    assert code == 0
    assert errors == ""
    assert payload["available"] is True
    assert payload["pending_episode_count"] == 1
    assert payload["selected_episode"]["episode_id"] == "cli-episode-1"
    assert "evaluation_input" not in json.dumps(payload)
    assert payload["authorization"] == "metadata_read_only; no_provider_or_credential_access"


def test_learning_status_missing_store_does_not_create_it(tmp_path) -> None:
    path = tmp_path / "missing-learning.sqlite3"
    code, payload, errors = _invoke("learning-status", "--learning-store", str(path))
    assert code == 0
    assert errors == ""
    assert payload["available"] is False
    assert not path.exists()


def test_settle_learning_accepts_only_a_value_only_decision_and_never_collects_credentials(tmp_path) -> None:
    path = tmp_path / "settle-learning.sqlite3"
    _write_cli_learning_episode(path)
    calls: list[tuple[str, dict[str, object]]] = []

    class Result:
        def require_ok(self):
            return {
                "ok": True,
                "status": "recorded_evaluator_reward",
                "learning_evidence": {"evidence_digest": "e" * 64},
                "next_state": {
                    "schema": "bioprism-brain-bandit/0.1",
                    "generation": 1,
                    "arms": [{"arm_id": "offline/test-model", "pulls": 1, "reward_sum": 0.8}],
                },
            }

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

        def call_tool(self, name: str, arguments=None):
            calls.append((name, dict(arguments or {})))
            return Result()

    code, payload, errors = _invoke(
        "settle-learning",
        "--learning-store", str(path),
        "--episode-id", "cli-episode-1",
        "--evaluator-id", "offline-evaluator",
        "--evaluator-version", "1",
        "--reward", "0.8",
        "--outcome", "passed",
        "--mcp-command", "python brain_server.py",
        client_factory=lambda *_args, **_kwargs: FakeClient(),
    )
    assert code == 0
    assert errors == ""
    assert payload["decision"]["reward"] == 0.8
    assert payload["pending_episode_count_after"] == 0
    assert payload["authorization"]["provider_call"] is False
    assert calls[0][0] == "brain_outcome_record"
    encoded = json.dumps(payload)
    assert "api_key" not in encoded
    assert "evaluation_input" not in encoded


def test_run_wires_opt_in_health_and_learning_ledgers_without_exposing_state_or_keys(tmp_path) -> None:
    captured: dict[str, object] = {}

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, **kwargs: object) -> None:
            captured.update(kwargs)

        def run(self, **kwargs: object) -> dict[str, object]:
            captured["run"] = kwargs
            return {"status": "completed"}

    secret = "state-wiring-secret-that-must-not-appear"
    output = io.StringIO()
    errors = io.StringIO()
    health_path = tmp_path / "health.jsonl"
    learning_path = tmp_path / "learning.sqlite"
    with patch("prism_sdk.cli.AutonomousAgent", FakeAgent):
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--domain", "science",
                "--task", "compare independent research sources",
                "--model", "model-a",
                "--provider", "openai",
                "--base-url", "https://provider.example",
                "--credential-source", "environment",
                "--credential-env", "AURORA_TEST_KEY",
                "--health-store", str(health_path),
                "--learning-store", str(learning_path),
                "--learning-mode", "online",
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
    assert captured["ledger"].path == learning_path
    assert captured["health_ledger"].path == health_path
    assert captured["run"]["learn"] is True
    assert payload["state_persistence"] == {
        "health_store_configured": True,
        "learning_store_configured": True,
        "learning_mode": "online",
    }
    assert secret not in output.getvalue()
    status_code, state_payload, state_errors = _invoke(
        "state-status",
        "--health-store", str(health_path),
        "--learning-store", str(learning_path),
    )
    assert status_code == 0
    assert state_errors == ""
    assert state_payload["learning"]["available"] is True
    assert set(state_payload["learning"]["domain_learning"]) == set(AUTONOMOUS_DOMAINS)


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
                "--learning-mode", "online",
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
    assert captured["learning_mode"] == "online"
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


def test_run_can_rehydrate_persisted_catalogue_without_provider_rediscovery(tmp_path) -> None:
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

    persisted = ModelCatalogue(
        (
            ModelCandidate(
                provider="openai",
                model="persisted-model",
                context_window_tokens=16_384,
                max_output_tokens=2_048,
                quality=0.81,
                latency_ms=180,
                cost_per_million_tokens=4,
                reliability=0.93,
                capabilities=("reasoning", "science"),
            ),
        )
    )
    secret = "persisted-run-secret-that-must-not-appear"
    output = io.StringIO()
    errors = io.StringIO()
    with (
        patch("prism_sdk.cli.AutonomousModelInventoryStore.load_catalogue", return_value=persisted),
        patch("prism_sdk.cli.AutonomousAgent", FakeAgent),
        patch("prism_sdk.cli.LLMRuntime.discover_models", side_effect=AssertionError("rediscovery")),
    ):
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--domain", "science",
                "--task", "compare independent research sources",
                "--use-inventory",
                "--inventory-store", str(tmp_path / "inventory.json"),
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
    assert [candidate.model for candidate in candidates] == ["persisted-model"]
    assert candidates[0].quality == 0.81
    assert payload["model_inventory"]["mode"] == "persisted_catalogue"
    assert payload["model_inventory"]["candidates"][0]["model"] == "persisted-model"
    assert secret not in output.getvalue()
