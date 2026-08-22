from __future__ import annotations

import io
import json
from pathlib import Path
import sys
from unittest.mock import patch

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousBatchCheckpoint,
    AutonomousBatchItem,
    AutonomousBatchResult,
    AutonomousBatchRehydrationContext,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomousWorkflowCheckpoint,
    BRAIN_LEARNING_EPISODE_SCHEMA,
    ModelCandidate,
    ModelCatalogue,
    ProviderModelDescriptor,
    SQLiteBrainLearningLedger,
)
from prism_sdk.authoring import content_digest
from prism_sdk.autonomy import _batch_digest
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


def test_explicit_local_provider_is_ready_without_a_credential_or_network() -> None:
    code, payload, errors = _invoke("provider-status", "--provider", "local")
    assert code == 0
    assert errors == ""
    assert payload["status"]["ready"] is True
    assert payload["status"]["requires_credential"] is False
    assert payload["instructions"]["next_action"] == "ready"
    assert payload["provider"]["transport"] == "in_memory"

    code, onboarded, errors = _invoke(
        "onboard",
        "--provider", "local",
        reader=lambda _prompt: (_ for _ in ()).throw(AssertionError("local onboarding prompted")),
    )
    assert code == 0
    assert errors == ""
    assert onboarded["provider"]["ready"] is True
    assert onboarded["session"]["providers"] == []


def test_local_provider_discovery_is_explicitly_approved_but_keyless() -> None:
    secret = "must-not-be-read-by-local-provider"
    code, payload, errors = _invoke(
        "discover-models",
        "--provider", "local",
        "--approve-provider-call",
        "--credential-source", "environment",
        "--credential-env", "SHOULD_NOT_BE_READ",
        environ={"SHOULD_NOT_BE_READ": secret},
    )
    assert code == 0
    assert errors == ""
    assert payload["models"][0]["model"] == "local-model"
    assert payload["credential_session"]["providers"] == []
    assert secret not in json.dumps(payload)


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


def test_execution_status_is_provider_free_and_projects_hash_verified_state(tmp_path) -> None:
    path = tmp_path / "executions.jsonl"
    journal = AutonomousExecutionJournal(path)
    journal.begin(
        execution_id="execution-cli-1",
        domain="coding",
        capability="implementation",
        risk_class="engineering_change",
        policy=AutonomousExecutionPolicy(),
    )
    code, payload, errors = _invoke(
        "execution-status",
        "--execution-store", str(path),
        "--execution-id", "execution-cli-1",
    )
    assert code == 0
    assert errors == ""
    assert payload["available"] is True
    assert payload["executions"][0]["state"]["execution_id"] == "execution-cli-1"
    assert payload["events"][0]["kind"] == "started"
    assert payload["authorization"] == "metadata_read_only; no_provider_or_credential_access"
    assert "implementation plan" not in json.dumps(payload)


def test_execution_status_missing_store_does_not_create_it(tmp_path) -> None:
    path = tmp_path / "missing-executions.jsonl"
    code, payload, errors = _invoke("execution-status", "--execution-store", str(path))
    assert code == 0
    assert errors == ""
    assert payload["available"] is False
    assert not path.exists()


def _cli_workflow_checkpoint() -> AutonomousWorkflowCheckpoint:
    return AutonomousWorkflowCheckpoint(
        run_id="workflow-cli-run-1",
        task_digest="a" * 64,
        workflow_id="coding-workflow",
        workflow_digest="b" * 64,
        stages=(
            {
                "stage_id": "scope",
                "status": "completed",
                "execution_status": "completed",
                "structured": {"summary": "caller-owned structured stage value"},
                "evidence": ["evidence-digest-or-label"],
                "uncertainty": [],
                "attempt": 1,
                "response_digest": "c" * 64,
                "stage_execution_plan_digest": "d" * 64,
                "stage_selected_tool_names": ["read_repository"],
                "stage_capability_contract_digests": ["e" * 64],
            },
            {
                "stage_id": "inspect",
                "status": "not_attempted",
                "execution_status": "paused",
                "structured": {},
                "evidence": [],
                "uncertainty": ["awaiting_scope"],
                "attempt": 1,
                "response_digest": "f" * 64,
                "stage_execution_plan_digest": None,
                "stage_selected_tool_names": [],
                "stage_capability_contract_digests": [],
            },
        ),
    )


def test_workflow_status_projects_digest_verified_stage_lifecycle_without_structured_values(tmp_path) -> None:
    from prism_sdk.cli import _persist_workflow_checkpoint

    path = tmp_path / "workflow-checkpoint.json"
    checkpoint = _cli_workflow_checkpoint()
    _persist_workflow_checkpoint(str(path), checkpoint)
    code, payload, errors = _invoke(
        "workflow-status",
        "--workflow-checkpoint-store", str(path),
    )
    assert code == 0
    assert errors == ""
    assert payload["available"] is True
    assert payload["checkpoint"]["checkpoint_digest"] == checkpoint.checkpoint_digest
    assert payload["checkpoint"]["completed_stage_ids"] == ["scope"]
    assert payload["checkpoint"]["remaining_stage_ids"] == ["inspect"]
    assert payload["checkpoint"]["stages"][0]["evidence_count"] == 1
    assert "caller-owned structured stage value" not in json.dumps(payload)
    assert all("structured" not in stage for stage in payload["checkpoint"]["stages"])


def test_workflow_status_missing_store_does_not_create_it(tmp_path) -> None:
    path = tmp_path / "missing-workflow-checkpoint.json"
    code, payload, errors = _invoke(
        "workflow-status",
        "--workflow-checkpoint-store", str(path),
    )
    assert code == 0
    assert errors == ""
    assert payload["available"] is False
    assert not path.exists()


def test_workflow_status_rejects_tampered_checkpoint_store(tmp_path) -> None:
    from prism_sdk.cli import _persist_workflow_checkpoint

    path = tmp_path / "tampered-workflow-checkpoint.json"
    _persist_workflow_checkpoint(str(path), _cli_workflow_checkpoint())
    raw = json.loads(path.read_text(encoding="utf-8"))
    raw["checkpoint"]["stages"][0]["status"] = "blocked"
    path.write_text(json.dumps(raw), encoding="utf-8")
    code, payload, errors = _invoke(
        "workflow-status",
        "--workflow-checkpoint-store", str(path),
    )
    assert code == 2
    assert payload is None
    assert "command failed" in errors


def test_run_automatic_workflow_persists_and_explicitly_rehydrates_checkpoint(tmp_path) -> None:
    captured: dict[str, object] = {}
    checkpoint = _cli_workflow_checkpoint()
    store = tmp_path / "run-workflow-checkpoint.json"

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeResult:
        def __init__(self, value: AutonomousWorkflowCheckpoint) -> None:
            self.checkpoint = value

        def to_dict(self):
            return {"status": "paused"}

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, *, model_catalogue: object) -> None:
            captured["catalogue"] = model_catalogue

        def run_auto(self, **kwargs: object) -> FakeResult:
            captured["run_auto"] = kwargs
            return FakeResult(checkpoint)

    with patch("prism_sdk.cli.AutonomousAgent", FakeAgent):
        output = io.StringIO()
        errors = io.StringIO()
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--automatic",
                "--single-domain",
                "--task", "resume a staged coding review",
                "--provider", "local",
                "--model", "local-model",
                "--workflow-execution",
                "--workflow-max-stage-calls", "2",
                "--workflow-checkpoint-store", str(store),
            ),
            environ={},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    payload = json.loads(output.getvalue())
    assert code == 0
    assert errors.getvalue() == ""
    assert payload["workflow"]["checkpoint_persisted"] is True
    assert payload["workflow"]["checkpoint_digest"] == checkpoint.checkpoint_digest
    assert "caller-owned structured stage value" not in output.getvalue()

    captured.clear()
    with patch("prism_sdk.cli.AutonomousAgent", FakeAgent):
        output = io.StringIO()
        errors = io.StringIO()
        code = main(
            (
                "run",
                "--mcp-command", "python server.py",
                "--automatic",
                "--single-domain",
                "--task", "resume a staged coding review",
                "--provider", "local",
                "--model", "local-model",
                "--workflow-execution",
                "--workflow-checkpoint-store", str(store),
                "--resume-workflow",
            ),
            environ={},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    assert code == 0
    assert errors.getvalue() == ""
    assert captured["run_auto"]["workflow_checkpoint"].checkpoint_digest == checkpoint.checkpoint_digest
    assert captured["run_auto"]["workflow_execution"] is True
    assert json.loads(output.getvalue())["workflow"]["checkpoint_loaded"] is True


def _cli_batch_checkpoint(
    *,
    result_digest: str,
    status: str = "completed",
    mode: str = "domain",
) -> AutonomousBatchCheckpoint:
    return AutonomousBatchCheckpoint(
        job_id="cli-batch-job-1",
        mode=mode,
        batch_input_digest="a" * 64,
        request_digests=("b" * 64,),
        completed_indices=(0,),
        completed_result_digests=(result_digest,),
        max_parallelism=1,
        stop_on_error=False,
        status=status,
    )


def test_batch_status_is_provider_free_and_projects_only_checkpoint_metadata(tmp_path) -> None:
    from prism_sdk.cli import _BatchCheckpointFileStore

    class Result:
        status = "completed"

    task_digest = content_digest({"task": "batch task"})
    item = AutonomousBatchItem(index=0, status="succeeded", task_digest=task_digest, result=Result())
    checkpoint = _cli_batch_checkpoint(result_digest=content_digest(item.to_dict()))
    path = tmp_path / "batch-checkpoint.json"
    _BatchCheckpointFileStore(str(path)).write(checkpoint)
    code, payload, errors = _invoke("batch-status", "--batch-checkpoint-store", str(path))
    assert code == 0
    assert errors == ""
    assert payload["available"] is True
    assert payload["checkpoint"]["job_id"] == "cli-batch-job-1"
    assert payload["checkpoint"]["completed_indices"] == [0]
    assert payload["checkpoint"]["checkpoint_digest"] == checkpoint.checkpoint_digest
    assert "batch task" not in json.dumps(payload)
    assert "options" not in json.dumps(payload)


def test_batch_status_missing_store_does_not_create_it(tmp_path) -> None:
    path = tmp_path / "missing-batch-checkpoint.json"
    code, payload, errors = _invoke("batch-status", "--batch-checkpoint-store", str(path))
    assert code == 0
    assert errors == ""
    assert payload["available"] is False
    assert not path.exists()


def test_batch_request_file_rejects_credential_shaped_fields_before_provider_access(tmp_path) -> None:
    requests = tmp_path / "requests.json"
    requests.write_text(
        json.dumps(
            {
                "schema": "aurora-autonomous-batch-requests/0.1",
                "mode": "domain",
                "job_id": "cli-batch-job-1",
                "requests": [
                    {
                        "task": "inspect all domains",
                        "domain": "coding",
                        "options": {"authorization": "must-never-enter-the-request"},
                    },
                ],
            }
        ),
        encoding="utf-8",
    )
    code, payload, errors = _invoke(
        "batch-run",
        "--mcp-command", "python server.py",
        "--requests-file", str(requests),
        "--job-id", "cli-batch-job-1",
        "--provider", "local",
        "--model", "local-model",
    )
    assert code == 2
    assert payload is None
    assert "command failed" in errors
    assert "must-never-enter-the-request" not in errors


def test_batch_run_wires_all_domain_request_file_controls_and_writes_status_manifest(tmp_path) -> None:
    captured: dict[str, object] = {}
    requests = tmp_path / "requests.json"
    requests.write_text(
        json.dumps(
            {
                "schema": "aurora-autonomous-batch-requests/0.1",
                "mode": "domain",
                "job_id": "cli-batch-job-1",
                "requests": [
                    {
                        "task": f"inspect {domain} evidence",
                        "domain": domain,
                        "options": {"max_steps": 3} if domain == "coding" else {},
                    }
                    for domain in AUTONOMOUS_DOMAINS
                ],
            }
        ),
        encoding="utf-8",
    )
    checkpoint_path = tmp_path / "batch-checkpoint.json"
    manifest_path = tmp_path / "batch-results.json"

    class Result:
        status = "completed"

    items = tuple(
        AutonomousBatchItem(
            index=index,
            status="succeeded",
            task_digest=content_digest({"task": f"inspect {domain} evidence"}),
            result=Result(),
        )
        for index, domain in enumerate(AUTONOMOUS_DOMAINS)
    )
    batch = AutonomousBatchResult(
        status="completed",
        items=items,
        completed_count=len(items),
        failed_count=0,
        omitted_count=0,
        max_parallelism=1,
        stop_on_error=False,
        batch_digest=_batch_digest(items),
    )
    checkpoint = AutonomousBatchCheckpoint(
        job_id="cli-batch-job-1",
        mode="domain",
        batch_input_digest="a" * 64,
        request_digests=tuple(f"{index:064x}" for index in range(len(items))),
        completed_indices=tuple(range(len(items))),
        completed_result_digests=tuple(content_digest(item.to_dict()) for item in items),
        max_parallelism=1,
        stop_on_error=False,
        status="completed",
    )

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, **kwargs: object) -> None:
            captured["agent_kwargs"] = kwargs

    class FakeController:
        def __init__(self, _agent: object, persistence: object) -> None:
            captured["persistence"] = persistence

        def restore(self):
            captured["restored"] = True
            return {"status": "empty"}

        def run(self, requests_value, **kwargs: object):
            captured["requests"] = requests_value
            captured["options"] = kwargs["options_factory"](requests_value[0], 0)
            captured["rehydrate_result"] = kwargs.get("rehydrate_result")
            persistence = captured["persistence"]
            persistence.write(checkpoint)
            return {"controller": {"status": "completed"}, "batch": batch}

    with (
        patch("prism_sdk.cli.AutonomousAgent", FakeAgent),
        patch("prism_sdk.cli.AutonomousBrainBatchJobController", FakeController),
    ):
        output = io.StringIO()
        errors = io.StringIO()
        code = main(
            (
                "batch-run",
                "--mcp-command", "python server.py",
                "--requests-file", str(requests),
                "--job-id", "cli-batch-job-1",
                "--provider", "local",
                "--model", "local-model",
                "--approve-provider-call",
                "--batch-checkpoint-store", str(checkpoint_path),
                "--batch-result-manifest", str(manifest_path),
                "--max-parallelism", "1",
            ),
            environ={},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    payload = json.loads(output.getvalue())
    assert code == 0
    assert errors.getvalue() == ""
    assert captured["restored"] is True
    assert {request["domain"] for request in captured["requests"]} == set(AUTONOMOUS_DOMAINS)
    assert captured["options"]["approve_provider_call"] is True
    assert captured["options"]["approve_mission_dispatch"] is False
    assert captured["options"]["max_steps"] == 3
    assert payload["batch"]["status"] == "completed"
    assert payload["batch_persistence"]["checkpoint_digest"] == checkpoint.checkpoint_digest
    assert manifest_path.exists()
    assert "inspect coding evidence" not in output.getvalue()


def test_batch_resume_requires_and_validates_status_only_manifest(tmp_path) -> None:
    from prism_sdk.cli import _BatchCheckpointFileStore, _load_batch_rehydrator, _write_batch_result_manifest

    class Result:
        status = "completed"

    task_digest = content_digest({"task": "batch task"})
    item = AutonomousBatchItem(index=0, status="succeeded", task_digest=task_digest, result=Result())
    batch = AutonomousBatchResult(
        status="completed",
        items=(item,),
        completed_count=1,
        failed_count=0,
        omitted_count=0,
        max_parallelism=1,
        stop_on_error=False,
        batch_digest=_batch_digest((item,)),
    )
    checkpoint = _cli_batch_checkpoint(
        result_digest=content_digest(item.to_dict()),
        mode="cross_domain",
    )
    checkpoint_path = tmp_path / "checkpoint.json"
    manifest_path = tmp_path / "manifest.json"
    _BatchCheckpointFileStore(str(checkpoint_path)).write(checkpoint)
    _write_batch_result_manifest(str(manifest_path), checkpoint, batch)
    rehydrate = _load_batch_rehydrator(str(manifest_path), checkpoint)
    restored = rehydrate(
        AutonomousBatchRehydrationContext(
            job_id=checkpoint.job_id,
            index=0,
            mode="cross_domain",
            request_digest=checkpoint.request_digests[0],
            task_digest=task_digest,
            expected_result_digest=checkpoint.completed_result_digests[0],
        )
    )
    assert restored.status == "completed"
    manifest_path.write_text(manifest_path.read_text(encoding="utf-8").replace("completed", "tampered"), encoding="utf-8")
    try:
        _load_batch_rehydrator(str(manifest_path), checkpoint)
    except ValueError:
        pass
    else:
        raise AssertionError("tampered batch manifest must be rejected")


def test_batch_run_rehydrates_cross_domain_items_without_reconstructing_provider_payloads(tmp_path) -> None:
    captured: dict[str, object] = {}
    task = "combine the coding and science reviews"
    requests = tmp_path / "cross-requests.json"
    requests.write_text(
        json.dumps(
            {
                "schema": "aurora-autonomous-batch-requests/0.1",
                "mode": "cross_domain",
                "job_id": "cli-batch-job-1",
                "requests": [
                    {
                        "task": task,
                        "subtasks": [
                            {"id": "coding", "task": "review implementation", "domain": "coding"},
                            {"id": "science", "task": "review evidence", "domain": "science"},
                        ],
                    },
                ],
            }
        ),
        encoding="utf-8",
    )
    class Result:
        status = "completed"

    task_digest = content_digest({"task": task})
    item = AutonomousBatchItem(index=0, status="succeeded", task_digest=task_digest, result=Result())
    batch = AutonomousBatchResult(
        status="completed",
        items=(item,),
        completed_count=1,
        failed_count=0,
        omitted_count=0,
        max_parallelism=1,
        stop_on_error=False,
        batch_digest=_batch_digest((item,)),
    )
    checkpoint = _cli_batch_checkpoint(
        result_digest=content_digest(item.to_dict()),
        mode="cross_domain",
    )
    checkpoint_path = tmp_path / "cross-checkpoint.json"
    manifest_path = tmp_path / "cross-manifest.json"
    from prism_sdk.cli import _BatchCheckpointFileStore, _write_batch_result_manifest

    _BatchCheckpointFileStore(str(checkpoint_path)).write(checkpoint)
    _write_batch_result_manifest(str(manifest_path), checkpoint, batch)

    class FakeClient:
        def __enter__(self):
            return self

        def __exit__(self, *_args: object) -> None:
            return None

    class FakeAgent:
        def __init__(self, _workspace: object, _runtime: object, **_kwargs: object) -> None:
            return None

    class FakeController:
        def __init__(self, _agent: object, persistence: object) -> None:
            captured["persistence"] = persistence

        def restore(self):
            captured["restored"] = True
            return {"status": "restored"}

        def run(self, requests_value, **kwargs: object):
            captured["requests"] = requests_value
            rehydrate = kwargs["rehydrate_result"]
            restored = rehydrate(
                AutonomousBatchRehydrationContext(
                    job_id="cli-batch-job-1",
                    index=0,
                    mode="cross_domain",
                    request_digest=checkpoint.request_digests[0],
                    task_digest=task_digest,
                    expected_result_digest=checkpoint.completed_result_digests[0],
                )
            )
            captured["rehydrated_status"] = restored.status
            captured["persistence"].write(checkpoint)
            return {"controller": {"status": "completed"}, "batch": batch}

    with (
        patch("prism_sdk.cli.AutonomousAgent", FakeAgent),
        patch("prism_sdk.cli.AutonomousBrainBatchJobController", FakeController),
    ):
        output = io.StringIO()
        errors = io.StringIO()
        code = main(
            (
                "batch-run",
                "--mcp-command", "python server.py",
                "--requests-file", str(requests),
                "--job-id", "cli-batch-job-1",
                "--provider", "local",
                "--model", "local-model",
                "--batch-checkpoint-store", str(checkpoint_path),
                "--batch-result-manifest", str(manifest_path),
                "--resume-batch",
            ),
            environ={},
            writer=output,
            error_writer=errors,
            client_factory=lambda *_args, **_kwargs: FakeClient(),
        )
    assert code == 0
    assert errors.getvalue() == ""
    assert captured["restored"] is True
    assert captured["rehydrated_status"] == "completed"
    assert captured["requests"][0]["subtasks"][1]["domain"] == "science"
    assert "combine the coding and science reviews" not in output.getvalue()


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
    execution_path = tmp_path / "executions.jsonl"
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
                "--execution-store", str(execution_path),
                "--execution-id", "cli-execution-1",
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
    assert captured["execution_journal"].path == execution_path
    assert captured["run"]["learn"] is True
    assert payload["state_persistence"] == {
        "health_store_configured": True,
        "learning_store_configured": True,
        "learning_mode": "online",
        "execution_store_configured": True,
        "execution_id": "cli-execution-1",
        "resume_execution": False,
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


def test_keyless_subprocess_tool_loop_discovers_and_executes_a_live_mcp_tool() -> None:
    fixture = Path(__file__).parent / "autonomous_brain_mcp_server.py"
    command = f'"{sys.executable.replace(chr(92), "/")}" -u "{fixture.as_posix()}"'
    response_sequence = json.dumps(
        [
            {
                "tool_calls": [
                    {
                        "id": "call-workspace-read",
                        "name": "workspace_read",
                        "arguments": {"path": "README.md"},
                    }
                ]
            },
            {"output_text": "workspace scan complete"},
        ],
        separators=(",", ":"),
    )
    code, payload, errors = _invoke(
        "run",
        "--mcp-command", command,
        "--task", "inspect the workspace evidence",
        "--domain", "coding",
        "--provider", "local",
        "--model", "local-model",
        "--model-capability", "reasoning",
        "--model-capability", "code",
        "--local-response-sequence-json", response_sequence,
        "--execution-mode", "tool_loop",
        "--approve-provider-call",
        "--approve-mission-dispatch",
    )
    assert code == 0
    assert errors == ""
    assert payload["result"]["status"] == "completed_provider_tool_loop"
    loop = payload["result"]["provider_loop"]
    assert loop["tool_calls"] == 1
    assert loop["final_response"]["text"] == "workspace scan complete"
    assert payload["provider_status"]["attempts"] == 2
    assert payload["credential_session"]["providers"] == []
    assert payload["secret_material"] == "never_returned"


def test_keyless_subprocess_batch_routes_every_builtin_domain(tmp_path) -> None:
    fixture = Path(__file__).parent / "autonomous_brain_mcp_server.py"
    command = f'"{sys.executable.replace(chr(92), "/")}" -u "{fixture.as_posix()}"'
    requests_path = tmp_path / "all-domain-requests.json"
    requests_path.write_text(
        json.dumps(
            {
                "schema": "aurora-autonomous-batch-requests/0.1",
                "mode": "domain",
                "job_id": "keyless-all-domain-001",
                "requests": [
                    {"task": f"produce bounded {domain} evidence", "domain": domain}
                    for domain in AUTONOMOUS_DOMAINS
                ],
            }
        ),
        encoding="utf-8",
    )
    response_sequence = json.dumps(
        [{"output_text": f"completed {domain}"} for domain in AUTONOMOUS_DOMAINS],
        separators=(",", ":"),
    )
    capabilities = (
        "reasoning", "code", "web", "data", "science", "biomedical", "operations",
        "enterprise", "coordination", "multimodal", "evaluation",
    )
    args = [
        "batch-run",
        "--mcp-command", command,
        "--requests-file", str(requests_path),
        "--job-id", "keyless-all-domain-001",
        "--provider", "local",
        "--model", "local-model",
        "--local-response-sequence-json", response_sequence,
        "--max-parallelism", "1",
        "--approve-provider-call",
    ]
    for capability in capabilities:
        args.extend(("--model-capability", capability))
    code, payload, errors = _invoke(*args)
    assert code == 0
    assert errors == ""
    assert payload["batch"]["status"] == "completed"
    assert payload["batch"]["completed_count"] == len(AUTONOMOUS_DOMAINS)
    assert payload["batch"]["failed_count"] == 0
    assert [item["status"] for item in payload["batch"]["items"]] == [
        "succeeded"
    ] * len(AUTONOMOUS_DOMAINS)
    assert payload["credential_session"]["providers"] == []


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
