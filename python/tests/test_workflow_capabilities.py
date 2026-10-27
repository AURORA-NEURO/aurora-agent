from dataclasses import replace
from unittest.mock import Mock
import socket

import pytest

from prism_sdk.errors import ArgumentError
from prism_sdk.workflow_capabilities import (
    WORKFLOW_CAPABILITY_IDS, compile_workflow_capability, validate_workflow_capabilities,
    with_workflow_capabilities, workflow_capability_inventory,
)
from prism_sdk.workflow_integrations import (
    WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult, builtin_workflow_operation_catalog,
)


@pytest.fixture(autouse=True)
def no_network(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("capability inspection or compilation attempted a network request")
    monkeypatch.setattr(socket, "getaddrinfo", denied)
    monkeypatch.setattr(socket, "create_connection", denied)


def _operation(name="local.inspect"):
    return WorkflowOperation(name, "Inspect local fixture", {"type": "object"}, {"type": "object"},
                             "local.fixture", implementation="local")


def _executor(marker):
    return lambda *_args, **_kwargs: WorkflowOperationResult({"marker": marker}, {})


@pytest.mark.parametrize("selection", [None, "gene-identity", {}, [True], [[]], ["unknown"],
    ["gene-identity", "gene-identity"], ["gene-identity"] * 33])
def test_every_capability_selection_is_validated_before_loading_extensions(monkeypatch, selection):
    import prism_sdk.workflow_capabilities as module
    load = Mock(side_effect=AssertionError("invalid selection loaded code"))
    monkeypatch.setattr(module, "_extension", load)
    catalog = builtin_workflow_operation_catalog()
    before = catalog.to_dict()
    with pytest.raises(ArgumentError):
        with_workflow_capabilities(catalog, selection)
    load.assert_not_called()
    assert catalog.to_dict() == before


def test_catalogue_subset_preserves_exact_executor_without_dispatch():
    operation = _operation()
    executor = Mock(wraps=_executor("retained"))
    original = WorkflowOperationCatalog([operation], {operation.operation_id: executor})
    selected = original.subset([operation.operation_id])
    assert selected.to_dict() == original.to_dict()
    executor.assert_not_called()
    assert selected.execute(operation.operation_id, {}).value == {"marker": "retained"}
    assert executor.call_count == 1
    assert not original.subset([]).operations()


@pytest.mark.parametrize("selection", [None, "local.inspect", ["missing"], ["local.inspect"] * 2, [[]], [True]])
def test_invalid_catalogue_subset_does_not_mutate_original(selection):
    operation = _operation()
    catalog = WorkflowOperationCatalog([operation], {operation.operation_id: _executor("original")})
    before = catalog.to_dict()
    with pytest.raises(ArgumentError):
        catalog.subset(selection)
    assert catalog.to_dict() == before


def test_matching_extension_preserves_caller_executor_and_does_not_promote_source_conformance():
    operation, added = _operation(), _operation("local.added")
    original = WorkflowOperationCatalog([operation], {operation.operation_id: _executor("caller")})
    extension = WorkflowOperationCatalog([operation, added], {
        operation.operation_id: _executor("extension"), added.operation_id: _executor("added")})
    combined = original.extended(extension)
    assert combined.execute(operation.operation_id, {}).value == {"marker": "caller"}
    assert combined.execute(added.operation_id, {}).value == {"marker": "added"}
    assert len(original.operations()) == 1 and len(extension.operations()) == 2


def test_descriptor_conflict_refuses_entire_extension_without_mutating_either_catalogue():
    original_op, added = _operation("local.z"), _operation("local.a")
    changed = replace(original_op, description="Different concrete contract")
    original = WorkflowOperationCatalog([original_op], {original_op.operation_id: _executor("original")})
    extension = WorkflowOperationCatalog([added, changed], {
        added.operation_id: _executor("added"), changed.operation_id: _executor("changed")})
    before = original.to_dict(), extension.to_dict()
    with pytest.raises(ArgumentError, match="conflict"):
        original.extended(extension)
    assert (original.to_dict(), extension.to_dict()) == before


@pytest.mark.parametrize("left,right", [(True, 1), (False, 0), (1, 1.0)])
def test_catalogue_extension_does_not_collapse_distinct_json_const_types(left, right):
    from prism_sdk.authoring import canonical_json
    first = replace(_operation(), input_schema={"type": "object", "properties": {"mode": {"const": left}}})
    second = replace(first, input_schema={"type": "object", "properties": {"mode": {"const": right}}})
    assert first.to_dict() == second.to_dict()  # Python equality alone erases the JSON type distinction.
    assert canonical_json(first.to_dict()) != canonical_json(second.to_dict())
    base = WorkflowOperationCatalog([first], {first.operation_id: _executor("original")})
    extension = WorkflowOperationCatalog([second], {second.operation_id: _executor("replacement")})
    with pytest.raises(ArgumentError, match="conflict"):
        base.extended(extension)
    assert base.resolve(first.operation_id).to_dict() == first.to_dict()


def test_profile_presence_requires_canonical_json_identity_instead_of_python_numeric_equality():
    from copy import deepcopy
    expanded = with_workflow_capabilities(builtin_workflow_operation_catalog(), ["glioma-ontology"])
    operation = expanded.resolve("nci_thesaurus.glioma_ontology")
    schema = deepcopy(operation.to_dict()["input_schema"])
    schema["properties"]["timeout_ms"]["default"] = 30000.0
    changed = replace(operation, input_schema=schema)
    assert changed.to_dict() == operation.to_dict()
    current = builtin_workflow_operation_catalog().extended(WorkflowOperationCatalog([changed],
        {changed.operation_id: _executor("not_admitted")}))
    inventory = workflow_capability_inventory(current, ["glioma-ontology"])["profiles"][1]
    assert not inventory["descriptor_available"] and inventory["mismatched_operations"] == [operation.operation_id]
    with pytest.raises(ArgumentError, match="unavailable or changed"):
        compile_workflow_capability("Inspect", current, "glioma-ontology", enabled_profiles=["glioma-ontology"])


def test_explicit_profiles_extend_only_concrete_operations_and_keep_default_catalogue_stable():
    base = builtin_workflow_operation_catalog()
    before = base.to_dict()
    expanded = with_workflow_capabilities(base, WORKFLOW_CAPABILITY_IDS)
    assert base.to_dict() == before == builtin_workflow_operation_catalog().to_dict()
    assert len(expanded.operations()) == 16  # eight defaults, four identity, one ontology, one CIViC reader and two curation locals
    again = with_workflow_capabilities(expanded, WORKFLOW_CAPABILITY_IDS)
    assert again.catalog_digest == expanded.catalog_digest
    inventory = workflow_capability_inventory(expanded, WORKFLOW_CAPABILITY_IDS)
    assert all(row["operator_enabled"] and row["descriptor_available"] for row in inventory["profiles"])
    assert all(row["source_availability"] == "unmeasured" for row in inventory["profiles"])
    disabled = workflow_capability_inventory(expanded, [])
    assert all(not row["operator_enabled"] and not row["descriptor_available"] for row in disabled["profiles"])
    assert inventory["inventory_unit"].endswith("not_a_provider_count")


@pytest.mark.parametrize("profile_id,steps", [("gene-identity", 5), ("glioma-ontology", 1), ("civic-glioblastoma-evidence", 1), ("civic-curation-review", 3)])
def test_profile_compilation_requires_operator_selection_and_binds_fixed_native_contract(profile_id, steps):
    task = "Inspect the documented fixed public metadata"
    catalog = with_workflow_capabilities(builtin_workflow_operation_catalog(), [profile_id])
    with pytest.raises(ArgumentError, match="not enabled"):
        compile_workflow_capability(task, catalog, profile_id, enabled_profiles=[])
    plan = compile_workflow_capability(task, catalog, profile_id, enabled_profiles=[profile_id]).to_dict()
    assert len(plan["steps"]) == steps and plan["requirements"]
    assert plan["outputs"] == [plan["steps"][-1]["id"]]
    assert plan["requirements"]["task_digest"]


def test_profile_presence_inspection_distinguishes_missing_and_changed_descriptors():
    base = builtin_workflow_operation_catalog()
    absent = workflow_capability_inventory(base, ["glioma-ontology"])["profiles"][1]
    assert absent["missing_operations"] == ["nci_thesaurus.glioma_ontology"]
    expanded = with_workflow_capabilities(base, ["glioma-ontology"])
    operation = expanded.resolve("nci_thesaurus.glioma_ontology")
    altered = base.extended(WorkflowOperationCatalog([replace(operation, description="different")],
        {operation.operation_id: _executor("never")}))
    item = workflow_capability_inventory(altered, ["glioma-ontology"])["profiles"][1]
    assert item["mismatched_operations"] == [operation.operation_id] and not item["descriptor_available"]
    with pytest.raises(ArgumentError, match="unavailable or changed"):
        compile_workflow_capability("Inspect", altered, "glioma-ontology", enabled_profiles=["glioma-ontology"])


def test_profile_extension_capacity_failure_is_atomic(monkeypatch):
    import prism_sdk.workflow_integrations as module
    base = builtin_workflow_operation_catalog()
    before = base.to_dict()
    monkeypatch.setattr(module, "MAX_WORKFLOW_OPERATIONS", 8)
    with pytest.raises(ArgumentError, match="full"):
        with_workflow_capabilities(base, ["glioma-ontology"])
    assert base.to_dict() == before


def test_profile_selection_is_canonical_without_erasing_duplicates():
    assert validate_workflow_capabilities(["glioma-ontology", "gene-identity"]) == ("gene-identity", "glioma-ontology")
    assert validate_workflow_capabilities(list(reversed(WORKFLOW_CAPABILITY_IDS))) == tuple(sorted(WORKFLOW_CAPABILITY_IDS))


def test_civic_profile_has_one_fixed_step_and_does_not_enable_source_authority():
    profile = "civic-glioblastoma-evidence"
    operation = "civic.glioblastoma_evidence_metadata"
    base = builtin_workflow_operation_catalog()
    assert operation not in {item.operation_id for item in base.operations()}
    expanded = with_workflow_capabilities(base, [profile])
    plan = compile_workflow_capability("Inspect fixed CIViC metadata", expanded, profile,
        enabled_profiles=[profile]).to_dict()
    assert plan["steps"] == [{"id": "evidence", "operation": operation,
        "arguments": {"timeout_ms": 30_000}, "depends_on": []}]
    assert plan["requirements"]["max_steps"] == 1
    assert plan["requirements"]["max_calls_by_operation"] == {operation: 1}
    inventory = workflow_capability_inventory(expanded, [profile])
    row = next(item for item in inventory["profiles"] if item["profile_id"] == profile)
    assert row["source_availability"] == "unmeasured"
    assert inventory["execution_authority"].startswith("none;")


@pytest.mark.parametrize("profile_id,steps", [("gene-identity", 5), ("glioma-ontology", 1), ("civic-glioblastoma-evidence", 1), ("civic-curation-review", 3)])
def test_worker_offers_only_startup_enabled_profiles_and_saves_native_plan_without_a_grant(tmp_path, monkeypatch, profile_id, steps):
    from prism_sdk import llm_runtime
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker, WorkflowAPIError
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("fixed profile constructed a model")))
    disabled = WorkflowAPIWorker(tmp_path / "disabled")
    disabled.catalog = with_workflow_capabilities(disabled.catalog, [profile_id])
    with pytest.raises(WorkflowAPIError, match="not supported"):
        disabled.handle({"action": "plan", "task": "Inspect native metadata", "planner": "capability:" + profile_id})
    enabled = WorkflowAPIWorker(tmp_path / "enabled", capabilities=(profile_id,))
    catalogue = enabled.handle({"action": "catalogue"})
    selected = next(row for row in catalogue["capabilities"]["profiles"] if row["profile_id"] == profile_id)
    assert selected["operator_enabled"] and selected["descriptor_available"]
    value = enabled.handle({"action": "plan", "task": "Inspect native metadata", "planner": "capability:" + profile_id})
    assert value["status"] == "planned" and value["source_dispatch"] is False
    assert value["interpretation"]["provider_calls"] == 0
    assert len(value["plan"]["steps"]) == steps
    assert not list((enabled.root / "runs").iterdir())


@pytest.mark.parametrize("selection", [("unknown",), ("glioma-ontology", "glioma-ontology"), "gene-identity"])
def test_invalid_worker_capability_configuration_refuses_before_creating_storage(tmp_path, selection):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker
    root = tmp_path / "absent"
    with pytest.raises(ArgumentError):
        WorkflowAPIWorker(root, capabilities=selection)
    assert not root.exists()


def test_plan_request_cannot_supply_a_capability_configuration_or_weaken_step_bound(tmp_path):
    from prism_sdk.workflow_api_worker import WorkflowAPIWorker, WorkflowAPIError
    worker = WorkflowAPIWorker(tmp_path / "state", capabilities=("gene-identity",))
    with pytest.raises(WorkflowAPIError):
        worker.handle({"action": "plan", "task": "Inspect", "planner": "capability:gene-identity", "capabilities": ["glioma-ontology"]})
    with pytest.raises(ValueError):
        worker.handle({"action": "plan", "task": "Inspect", "planner": "capability:gene-identity", "max_steps": 4})
    assert not list((worker.root / "plans").iterdir()) and not list((worker.root / "runs").iterdir())


@pytest.mark.parametrize("profile_id,steps", [("gene-identity", 5), ("glioma-ontology", 1), ("civic-glioblastoma-evidence", 1), ("civic-curation-review", 3)])
def test_cli_fixed_template_selects_reviewed_profile_without_provider_or_source_grants(tmp_path, profile_id, steps):
    import json
    from test_workflow_cli import _invoke
    destination = tmp_path / "plan.json"
    code, payload, errors = _invoke("workflow-template", "--template", profile_id,
        "--task", "Inspect fixed native public metadata", "--out", str(destination))
    assert code == 0 and not errors and payload["source_dispatch"] is False
    plan = json.loads(destination.read_text(encoding="utf-8"))
    assert len(plan["steps"]) == steps and plan["requirements"]
    code, catalogue, errors = _invoke("workflow-catalogue", "--capability", profile_id)
    assert code == 0 and not errors and catalogue["capabilities"]["selected_profiles"] == [profile_id]


def test_actual_stdio_worker_passes_repeated_profile_arguments_and_never_accepts_request_configuration(tmp_path):
    from test_workflow_api_worker import _invoke
    root = tmp_path / "stdio-state"
    process, envelope = _invoke(root, {"action": "catalogue"}, flags=("--capability", "gene-identity", "--capability", "glioma-ontology"))
    assert process.returncode == 0 and not process.stderr and envelope["ok"]
    assert envelope["data"]["capabilities"]["selected_profiles"] == ["gene-identity", "glioma-ontology"]
    process, envelope = _invoke(root, {"action": "plan", "task": "Inspect", "planner": "capability:glioma-ontology"})
    assert process.returncode == 2 and not envelope["ok"]
    assert not list((root / "plans").iterdir()) and not list((root / "runs").iterdir())


@pytest.mark.parametrize("schema", ["aurora-glioma-denominator-review/0.1", "aurora-glioma-gene-identity-review/0.1"])
def test_run_summary_keeps_native_denominator_and_identity_scientific_review_holds(schema):
    from prism_sdk.workflow_cli import _output_review_statuses
    value = {"schema": schema, "status": "awaiting_scientific_human_review", "human_review_required": True,
             "scientific_support_established": False, "clinical_actions_authorized": False}
    assert _output_review_statuses({"review": value}) == {"review": value}
    value["status"] = "unrecognized_not_a_pass"
    assert "status" not in _output_review_statuses({"review": value})["review"]

