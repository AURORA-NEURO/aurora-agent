"""Actual CLI interpretation/cancellation boundaries, with no external model/source calls."""
import json
from unittest.mock import Mock

import pytest

from prism_sdk import workflow_cli
from prism_sdk.workflow_builder import load_workflow_checkpoint
from prism_sdk.workflow_requirements import compile_workflow_requirements
from test_workflow_cli import _fixture_catalog, _invoke, _plan_file, _proposal, _provider


def test_interpret_command_exposes_source_subset_without_constructing_any_runtime(monkeypatch):
    constructor = Mock(side_effect=AssertionError("interpretation is pure"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", constructor)
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    code, payload, errors = _invoke("workflow-interpret", "--task", "Review glioma evidence using PubMed only; exclude trials and TCGA.")
    assert code == 0 and errors == "" and payload["status"] == "ready"
    assert payload["source_operations"] == ["pubmed.search"]
    assert payload["provider_calls"] == 0 and payload["source_dispatch"] is False
    assert constructor.call_count == 0


def test_typed_plan_compiles_unequal_limits_and_actual_source_value_flow_without_provider(monkeypatch, tmp_path):
    monkeypatch.setattr(workflow_cli, "LLMRuntime", Mock(side_effect=AssertionError("typed planner does not use a model")))
    target = tmp_path / "typed.json"
    task = "Review glioma evidence using PubMed and trials; PubMed limit 2; trials page size 3; trials max pages 1."
    code, payload, errors = _invoke("workflow-build", "--planner", "glioma-intent", "--task", task, "--out", str(target))
    assert code == 0 and errors == "" and payload["status"] == "planned"
    plan = json.loads(target.read_text())
    source_arguments = {step["operation"]: step["arguments"] for step in plan["steps"]}
    assert source_arguments["pubmed.search"]["per_specialty_limit"] == 2
    assert source_arguments["clinical_trials.search"]["page_size"] == 3
    assert source_arguments["clinical_trials.search"]["max_pages"] == 1
    assert "gdc.projects" not in source_arguments
    assert plan["requirements"]["output_operations"] == ["glioma.evidence.review_handoff"]
    assert len(plan["requirements"]["data_links"]) == 3
    assert payload["interpretation"]["source_operations"] == ["pubmed.search", "clinical_trials.search"]


def test_unresolved_filters_never_write_or_dispatch_a_broader_plan(monkeypatch, tmp_path):
    constructor = Mock(side_effect=AssertionError("unresolved request must predate external setup"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    monkeypatch.setattr(workflow_cli, "LLMRuntime", constructor)
    target = tmp_path / "must-not-exist.json"
    code, payload, errors = _invoke("workflow-run", "--planner", "glioma-intent", "--task", "Review glioma evidence published after 2024.",
                                  "--approve-source-dispatch", "--out", str(target))
    assert code == 1 and errors == "" and payload["status"] == "needs_clarification"
    assert payload["source_dispatch"] is False and payload["interpretation"]["unresolved_spans"]
    assert not target.exists() and constructor.call_count == 0


def test_additional_contract_cannot_weaken_typed_source_exclusions(monkeypatch, tmp_path):
    monkeypatch.setattr(workflow_cli, "LLMRuntime", Mock(side_effect=AssertionError("typed planner is pure")))
    task = "Review glioma evidence using PubMed only; exclude trials and TCGA."
    caller = compile_workflow_requirements(task, required_operations=["clinical_trials.search"])
    contract_file, target = tmp_path / "caller.json", tmp_path / "plan.json"
    contract_file.write_text(json.dumps(caller.to_dict()))
    code, payload, errors = _invoke("workflow-build", "--planner", "glioma-intent", "--task", task,
                                  "--requirements", str(contract_file), "--out", str(target))
    assert code == 2 and payload is None and errors and not target.exists()


def test_matching_additional_contract_is_persisted_with_derived_contract(monkeypatch, tmp_path):
    monkeypatch.setattr(workflow_cli, "LLMRuntime", Mock(side_effect=AssertionError("typed planner is pure")))
    task = "Review glioma evidence using PubMed only; exclude trials and TCGA."
    caller = compile_workflow_requirements(task, argument_rules=[
        {"operation": "pubmed.search", "path": "/per_specialty_limit", "comparison": "at_most", "value": 12}])
    contract_file, target = tmp_path / "caller.json", tmp_path / "plan.json"
    contract_file.write_text(json.dumps(caller.to_dict()))
    code, payload, errors = _invoke("workflow-build", "--planner", "glioma-intent", "--task", task,
                                  "--requirements", str(contract_file), "--out", str(target))
    assert code == 0 and errors == "" and payload["status"] == "planned"
    rules = json.loads(target.read_text())["requirements"]["argument_rules"]
    assert caller.to_dict()["argument_rules"][0] in rules
    assert any(rule["operation"] == "pubmed.search" and rule["comparison"] == "equals" for rule in rules)


@pytest.mark.parametrize("extra", [["--max-steps", "2"], ["--operation", "pubmed.search"], ["--planning-attempts", "2"]])
def test_typed_planning_still_obeys_explicit_operator_budgets(monkeypatch, tmp_path, extra):
    monkeypatch.setattr(workflow_cli, "LLMRuntime", Mock(side_effect=AssertionError("typed planner is pure")))
    target = tmp_path / "refused.json"
    code, payload, errors = _invoke("workflow-build", "--planner", "glioma-intent", "--task", "Review public glioma evidence.",
                                  "--out", str(target), *extra)
    assert code == 2 and payload is None and errors and not target.exists()


def test_existing_cancel_signal_prevents_first_source_dispatch_and_is_durable(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    signal, checkpoint = tmp_path / "cancel.signal", tmp_path / "checkpoint.json"
    signal.touch()
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--cancel-file", str(signal),
                                  "--checkpoint-store", str(checkpoint))
    assert code == 130 and errors == "" and payload["status"] == "cancelled" and calls == []
    assert load_workflow_checkpoint(checkpoint)["cancel_requested"] is True


def test_mid_call_signal_drains_result_then_holds_remaining_steps_until_explicit_release(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory, second=True)
    signal, checkpoint = tmp_path / "cancel.signal", tmp_path / "checkpoint.json"
    state["before_execute"] = signal.touch
    base = ("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--cancel-file", str(signal), "--checkpoint-store", str(checkpoint))
    code, payload, errors = _invoke(*base)
    assert code == 130 and errors == "" and payload["status"] == "cancelled"
    assert list(payload["results"]) == ["first"] and len(calls) == 1
    signal.unlink()
    state["before_execute"] = None
    code, payload, _ = _invoke(*base, "--resume")
    assert code == 130 and payload["status"] == "cancelled" and len(calls) == 1
    code, payload, errors = _invoke(*base, "--resume", "--resume-cancelled")
    assert code == 0 and errors == "" and payload["status"] == "completed" and len(calls) == 2


def test_cancelled_source_failure_still_requires_explicit_retry_on_release(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    signal, checkpoint = tmp_path / "cancel.signal", tmp_path / "checkpoint.json"
    state.update(fail=True, before_execute=signal.touch)
    base = ("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--cancel-file", str(signal), "--checkpoint-store", str(checkpoint))
    code, payload, _ = _invoke(*base)
    assert code == 130 and payload["status"] == "cancelled" and payload["failure"]["step_id"] == "first"
    signal.unlink()
    state.update(fail=False, before_execute=None)
    code, payload, _ = _invoke(*base, "--resume", "--resume-cancelled")
    assert code == 1 and payload["status"] == "retry_required" and len(calls) == 1
    code, payload, _ = _invoke(*base, "--resume", "--resume-cancelled", "--retry-failed")
    assert code == 0 and payload["status"] == "completed" and len(calls) == 2


@pytest.mark.parametrize("signal_role", ["plan", "checkpoint", "outputs", "directory", "missing_parent"])
def test_cancel_signal_path_is_distinct_and_valid_before_dispatch(monkeypatch, tmp_path, signal_role):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    checkpoint, outputs = tmp_path / "run.json", tmp_path / "values.json"
    signal = {"plan": plan, "checkpoint": checkpoint, "outputs": outputs, "directory": tmp_path,
              "missing_parent": tmp_path / "missing" / "signal"}[signal_role]
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--cancel-file", str(signal),
                                  "--checkpoint-store", str(checkpoint), "--outputs-file", str(outputs))
    assert code == 2 and payload is None and errors and calls == []
    assert not checkpoint.exists() and not outputs.exists()


def test_cancellation_release_requires_checkpoint_resume_before_dispatch(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--resume-cancelled")
    assert code == 2 and payload is None and errors and calls == []


@pytest.mark.parametrize("budget", ["0", "33"])
def test_bad_run_dispatch_budget_precedes_provider_and_cannot_overwrite_plan(monkeypatch, tmp_path, budget):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    target = tmp_path / "reviewed-plan.json"
    target.write_text("operator-selected existing artifact")
    before = target.read_bytes()
    code, payload, errors = _invoke("workflow-run", "--task", "Read fixture metadata", "--model", "fixture-model",
        "--approve-provider-call", "--approve-source-dispatch", "--out", str(target), "--max-step-calls", budget)
    assert code == 2 and payload is None and errors and requests == calls == []
    assert target.read_bytes() == before


def test_molecular_template_executes_real_adapter_parsers_and_exposes_held_review_metadata(monkeypatch, tmp_path):
    from test_glioma_molecular_workflow import _fixture_adapters
    monkeypatch.setattr(workflow_cli, "LLMRuntime", Mock(side_effect=AssertionError("molecular catalogue analysis is model-free")))
    plan, outputs = tmp_path / "molecular-plan.json", tmp_path / "public-values.json"
    code, payload, errors = _invoke("workflow-template", "--template", "molecular", "--task", "Review public glioma molecular metadata",
        "--association-page-size", "1", "--gwas-page-size", "1", "--out", str(plan))
    assert code == 0 and errors == "" and payload["step_count"] == 6
    with _fixture_adapters() as calls:
        code, payload, errors = _invoke("workflow-execute", "--molecular-glioma", "--plan", str(plan),
            "--approve-source-dispatch", "--outputs-file", str(outputs), "--retrieved-at", "2026-10-26T12:00:00Z")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert all(calls.values())
    assert payload["output_review_statuses"]["review"] == {
        "schema": "aurora-glioma-molecular-review/0.1", "status": "awaiting_scientific_human_review",
        "human_review_required": True, "scientific_support_established": False, "clinical_actions_authorized": False,
    }
    assert "NCBI_Entrez_Gene" not in json.dumps(payload)
    assert json.loads(outputs.read_text())["outputs"]["review"]["coverage"]["measurement_values_retrieved"] is False


@pytest.mark.parametrize("template,options", [
    ("molecular", ["--no-tcga"]), ("molecular", ["--literature-limit", "2"]),
    ("evidence", ["--no-gwas"]), ("evidence", ["--association-page-size", "1"]),
    ("molecular", ["--grounded-glioma", "--model", "unused"]),
])
def test_template_specific_selectors_cannot_be_silently_ignored(monkeypatch, tmp_path, template, options):
    constructor = Mock(side_effect=AssertionError("unsupported option combination must predate model/catalogue setup"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", constructor)
    monkeypatch.setattr(workflow_cli, "workflow_cli_catalog", constructor)
    target = tmp_path / "must-not-exist.json"
    code, payload, errors = _invoke("workflow-template", "--template", template, "--task", "Review public glioma metadata",
                                  "--out", str(target), *options)
    assert code == 2 and payload is None and errors and not target.exists() and constructor.call_count == 0
