from __future__ import annotations

from copy import deepcopy
from dataclasses import FrozenInstanceError, replace
import json
from pathlib import Path
import socket

import pytest

from prism_sdk.authoring import canonical_json, content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk import reviewed_research_sources as research
from prism_sdk import workflow_provider_registry as registry_module
from prism_sdk.workflow_integrations import (
    WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult, builtin_workflow_operation_catalog,
)
from prism_sdk.workflow_provider_registry import (
    WorkflowProviderDispatchError, WorkflowProviderEvidenceReference, WorkflowProviderIdentity,
    WorkflowProviderRegistry, WorkflowProviderRequest, WorkflowReaderContract,
    reviewed_workflow_provider_registry,
)


def _all_catalog():
    return registry_module._reviewed_catalog()


def _reader(document, operation_id):
    return next(value for value in document["readers"] if value["operation_id"] == operation_id)


def _local_operation(name="fixture.local"):
    return WorkflowOperation(name, "Local fixture", {"type": "object"}, {"type": "object"}, "fixture", implementation="local")


def _reconstruct(registry, **changes):
    document = json.loads(registry._manifest)
    providers = tuple(WorkflowProviderIdentity(value["provider_id"], value["display_name"], tuple(value["origins"]), value["review_basis"]) for value in document["providers"])
    readers = tuple(WorkflowReaderContract(value["operation_id"], value["provider_id"], value["operation"], value["component_sha256"], value["review_document"]) for value in document["readers"])
    fields = {"catalog": registry._catalog, "providers": providers, "readers": readers,
              "reviewed_catalog": _all_catalog()}
    fields.update(changes)
    return WorkflowProviderRegistry(**fields)


def test_twenty_implemented_readers_are_not_twenty_providers_or_default_registration(monkeypatch):
    monkeypatch.setattr(socket, "getaddrinfo", lambda *_args, **_kwargs: pytest.fail("inspection attempted DNS"))
    monkeypatch.setattr(socket, "create_connection", lambda *_args, **_kwargs: pytest.fail("inspection opened a socket"))
    catalog = builtin_workflow_operation_catalog()
    before = catalog.to_dict()
    document = reviewed_workflow_provider_registry(catalog).to_dict()
    assert catalog.to_dict() == before
    assert document["counts"] == {
        "declared_reviewed_service_groups": 17, "implemented_reader_paths": 20,
        "registered_reviewed_default_readers": 8, "supported_readers_not_registered": 12,
        "registered_unmatched_readers": 0, "registered_reviewed_service_groups": 7,
        "external_evidence_references": 0,
        "independently_live_verified_providers": {"state": "not_established_by_registry", "value": None},
    }
    assert document["network_requests_on_inspection"] == 0
    assert document["unreviewed_operation_ids"] == []
    assert {value["registration_state"] for value in document["readers"]} == {"registered_reviewed_default", "not_registered"}
    assert all(value["live_observation"] == {"state": "unmeasured_by_registry", "value": None} for value in document["readers"])
    assert all(value["continuous_availability"] == "unmeasured" for value in document["readers"])


def test_all_real_optional_registrations_bind_without_capacity_counts_or_dispatch(monkeypatch):
    monkeypatch.setattr(socket, "getaddrinfo", lambda *_args, **_kwargs: pytest.fail("registration attempted DNS"))
    document = reviewed_workflow_provider_registry(_all_catalog()).to_dict()
    assert document["counts"]["registered_reviewed_default_readers"] == 20
    assert document["counts"]["registered_reviewed_service_groups"] == 17
    assert document["counts"]["supported_readers_not_registered"] == 0
    assert all(value["registration_state"] == "registered_reviewed_default" for value in document["readers"])
    assert "225" not in json.dumps(document["counts"])
    assert "4096" not in json.dumps(document["counts"])


def test_shared_service_readers_are_deduplicated_without_claiming_host_independence():
    document = reviewed_workflow_provider_registry(_all_catalog()).to_dict()
    assert _reader(document, "pubmed.search")["provider_id"] == _reader(document, "ncbi_gene.summary")["provider_id"] == "ncbi_eutils"
    assert _reader(document, "gdc.projects")["provider_id"] == _reader(document, "gdc.gene_mutation_counts")["provider_id"] == "gdc"
    groups = {value["provider_id"]: value for value in document["providers"]}
    assert groups["europe_pmc"]["origins"] == groups["gwas_catalog"]["origins"] == ["https://www.ebi.ac.uk"]
    assert groups["europe_pmc"]["operator_independence"] == "unassessed"
    assert groups["gwas_catalog"]["operator_independence"] == "unassessed"


def test_component_versions_and_exact_normalized_contracts_are_digest_bound():
    catalog = _all_catalog()
    registry = reviewed_workflow_provider_registry(catalog)
    document = registry.to_dict()
    assert document == reviewed_workflow_provider_registry(catalog).to_dict()
    for reader in document["readers"]:
        assert reader["operation"] == catalog.resolve(reader["operation_id"]).to_dict()
        assert reader["operation_digest"] == content_digest(reader["operation"])
        assert reader["upstream_contract_version"] == {"state": "not_established", "value": None}
        assert reader["component_sha256"]
        for name, digest in reader["component_sha256"].items():
            assert digest == registry_module._component_digest(name)
    assert "reviewed_public_http" in _reader(document, "uniprot.protein_annotation")["component_sha256"]
    assert "reviewed_public_http" in _reader(document, "reactome.pathway_annotations")["component_sha256"]
    assert "reviewed_public_http" not in _reader(document, "gdc.projects")["component_sha256"]


def test_civic_optional_default_contract_binds_both_transport_components_without_claiming_live_coverage(monkeypatch):
    from prism_sdk.workflow_capabilities import with_workflow_capabilities
    from prism_sdk import reviewed_public_graphql
    monkeypatch.setattr(reviewed_public_graphql.ReviewedPublicGraphqlClient, "post",
        lambda *_args, **_kwargs: pytest.fail("registry inspection dispatched CIViC"))
    base = builtin_workflow_operation_catalog()
    before = reviewed_workflow_provider_registry(base).to_dict()
    assert before["manifest_version"] == "reviewed-public-readers/2026-10-27.4"
    assert _reader(before, "civic.glioblastoma_evidence_metadata")["registration_state"] == "not_registered"
    expanded = with_workflow_capabilities(base, ["civic-glioblastoma-evidence"])
    registry = reviewed_workflow_provider_registry(expanded)
    reader = _reader(registry.to_dict(), "civic.glioblastoma_evidence_metadata")
    assert reader["registration_state"] == "registered_reviewed_default"
    assert {"reviewed_public_http", "reviewed_public_graphql", "reviewed_civic_evidence_retrieval"}.issubset(reader["component_sha256"])
    assert registry.prepare(reader["operation_id"], {}).to_dict()["arguments"] == {"timeout_ms": 30_000}
    assert reader["live_observation"] == {"state": "unmeasured_by_registry", "value": None}
    assert before["counts"]["independently_live_verified_providers"]["value"] is None


def test_civic_caller_transport_cannot_inherit_a_reviewed_default_reader_contract():
    from prism_sdk.reviewed_civic_evidence_retrieval import register_civic_evidence_operations
    def caller(*_args, **_kwargs):
        pytest.fail("registration entered caller transport")
    catalog = register_civic_evidence_operations(builtin_workflow_operation_catalog(), fetcher=caller)
    registry = reviewed_workflow_provider_registry(catalog)
    row = _reader(registry.to_dict(), "civic.glioblastoma_evidence_metadata")
    assert row["registration_state"] != "registered_reviewed_default"
    with pytest.raises(ArgumentError, match="exact registered reviewed default"):
        registry.prepare(row["operation_id"], {})


def test_every_review_document_exists_in_the_checkout():
    root = Path(__file__).parents[2]
    document = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog()).to_dict()
    assert all((root / value["review_document"]).is_file() for value in document["readers"])


def test_cellosaurus_optional_contract_binds_existing_get_policy_without_promoting_format_capture_to_live_coverage(monkeypatch):
    from prism_sdk.workflow_capabilities import with_workflow_capabilities
    from prism_sdk import reviewed_public_http
    monkeypatch.setattr(reviewed_public_http.ReviewedPublicHttpClient, "get",
        lambda *_args, **_kwargs: pytest.fail("onboarding inspection dispatched Cellosaurus"))
    base = builtin_workflow_operation_catalog()
    before = reviewed_workflow_provider_registry(base).to_dict()
    assert before["manifest_version"] == "reviewed-public-readers/2026-10-27.4"
    operation = "cellosaurus.glioblastoma_cell_line_metadata"
    assert _reader(before, operation)["registration_state"] == "not_registered"
    expanded = with_workflow_capabilities(base, ["cellosaurus-glioblastoma-metadata"])
    registry = reviewed_workflow_provider_registry(expanded)
    reader = _reader(registry.to_dict(), operation)
    assert reader["registration_state"] == "registered_reviewed_default"
    assert {"reviewed_public_http", "reviewed_cellosaurus_cell_line_retrieval"}.issubset(reader["component_sha256"])
    assert "reviewed_public_graphql" not in reader["component_sha256"]
    assert registry.prepare(operation, {}).to_dict()["arguments"] == {"timeout_ms": 30_000}
    assert reader["live_observation"] == {"state": "unmeasured_by_registry", "value": None}
    assert before["counts"]["independently_live_verified_providers"]["value"] is None


def test_cellosaurus_caller_fixture_binding_cannot_prepare_as_a_default_reader():
    from prism_sdk.reviewed_cellosaurus_cell_line_retrieval import register_cellosaurus_cell_line_operations
    catalog = register_cellosaurus_cell_line_operations(builtin_workflow_operation_catalog(),
        fetcher=lambda *_args, **_kwargs: pytest.fail("registration dispatched"))
    registry = reviewed_workflow_provider_registry(catalog)
    operation = "cellosaurus.glioblastoma_cell_line_metadata"
    assert _reader(registry.to_dict(), operation)["registration_state"] == "registered_contract_differs"
    with pytest.raises(ArgumentError, match="exact registered reviewed default"):
        registry.prepare(operation, {})


def test_cellosaurus_onboarding_preserves_all_nineteen_previous_descriptors_and_existing_get_policy():
    from prism_sdk.reviewed_public_http import REVIEWED_PUBLIC_HTTP_POLICY_DIGEST
    reviewed = _all_catalog()
    legacy = reviewed.subset([item.operation_id for item in reviewed.operations()
        if item.operation_id != "cellosaurus.glioblastoma_cell_line_metadata"])
    # Frozen offline from clean20bc before this onboarding slice; not regenerated from the new manifest.
    assert len(legacy.operations()) == 19
    assert legacy.catalog_digest == "636b5792f049bb121343980329d25950a066cc2b214cf1f022538710f6c92838"
    assert REVIEWED_PUBLIC_HTTP_POLICY_DIGEST == "07c73dc8fcba4fcdfa2cb08a941062f6a736cbcff38c02cdf457a09593a3f46e"


def test_inspection_exports_detached_copies_and_record_fields_are_frozen():
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    before = registry.to_dict()
    changed = registry.to_dict()
    changed["readers"][0]["operation"]["input_schema"] = {"arbitrary": True}
    changed["providers"][0]["origins"].append("https://unreviewed.invalid")
    assert registry.to_dict() == before
    provider = WorkflowProviderIdentity("service", "Service", ("https://api.example.org",), "Operator-reviewed fixture identity")
    with pytest.raises(FrozenInstanceError):
        provider.provider_id = "changed"
    first = provider.to_dict()
    first["origins"].clear()
    assert provider.to_dict()["origins"] == ["https://api.example.org"]
    raw = json.loads(registry._manifest)["readers"][0]
    contract = WorkflowReaderContract(raw["operation_id"], raw["provider_id"], raw["operation"], raw["component_sha256"], raw["review_document"])
    with pytest.raises(TypeError):
        contract.operation["input_schema"]["properties"]["arbitrary"] = {}
    with pytest.raises(TypeError):
        contract.components["authoring"] = "0" * 64
    raw["operation"]["description"] = "external mutation"
    assert contract.to_dict()["operation"]["description"] != "external mutation"


def test_unknown_local_mcp_openapi_and_named_aliases_do_not_inherit_reader_coverage():
    catalog = builtin_workflow_operation_catalog()
    for name in ("local.report", "mcp.fixture", "openapi.fixture", "pubmed.alias"):
        catalog.register(_local_operation(name), lambda *_args, **_kwargs: WorkflowOperationResult({}, {}))
    document = reviewed_workflow_provider_registry(catalog).to_dict()
    assert document["unreviewed_operation_ids"] == ["local.report", "mcp.fixture", "openapi.fixture", "pubmed.alias"]
    assert document["counts"]["registered_reviewed_default_readers"] == 8
    assert document["counts"]["declared_reviewed_service_groups"] == 17


def test_reusing_every_reviewed_descriptor_with_fixture_executors_establishes_no_reviewed_runtime_binding():
    original = _all_catalog()
    catalog = WorkflowOperationCatalog()
    calls = []
    for operation in original.operations():
        catalog.register(operation, lambda *_args, **_kwargs: calls.append(1))
    registry = reviewed_workflow_provider_registry(catalog)
    document = registry.to_dict()
    assert document["counts"]["registered_reviewed_default_readers"] == 0
    assert document["counts"]["registered_unmatched_readers"] == 20
    assert {value["registration_state"] for value in document["readers"]} == {"registered_executor_not_reviewed_default"}
    with pytest.raises(ArgumentError, match="exact registered"):
        registry.prepare("pubmed.search", {})
    assert calls == []


def test_hidden_caller_fetcher_in_research_keyword_defaults_is_not_a_reviewed_default():
    calls = []
    catalog = builtin_workflow_operation_catalog()
    research.register_research_source_operations(catalog, fetchers={"openalex": lambda *_args, **_kwargs: calls.append(1)})
    document = reviewed_workflow_provider_registry(catalog).to_dict()
    assert _reader(document, "openalex.search_works")["registration_state"] == "registered_executor_not_reviewed_default"
    assert _reader(document, "crossref.search_works")["registration_state"] == "registered_reviewed_default"
    assert calls == []


def test_caller_supplied_transport_and_changed_contract_are_visible_as_unmatched():
    from prism_sdk.reviewed_uniprot_retrieval import register_uniprot_operations
    catalog = builtin_workflow_operation_catalog()
    register_uniprot_operations(catalog, fetcher=lambda *_args, **_kwargs: pytest.fail("fixture dispatched"))
    operation = catalog.resolve("gdc.projects")
    catalog._operations[operation.operation_id] = replace(operation, description="Changed reviewed limits require a new contract")
    document = reviewed_workflow_provider_registry(catalog).to_dict()
    assert _reader(document, "uniprot.protein_annotation")["registration_state"] == "registered_contract_differs"
    assert _reader(document, "gdc.projects")["registration_state"] == "registered_contract_differs"
    assert document["counts"]["registered_unmatched_readers"] == 2


@pytest.mark.parametrize("mutation", ["addition", "executor", "descriptor", "closure", "keyword_defaults"])
def test_runtime_catalogue_and_executor_changes_require_a_new_binding_before_inspection_or_dispatch(mutation):
    catalog = _all_catalog()
    registry = reviewed_workflow_provider_registry(catalog)
    request = registry.prepare("pubmed.search", {})
    if mutation == "addition":
        catalog.register(_local_operation(), lambda *_args, **_kwargs: WorkflowOperationResult({}, {}))
    elif mutation == "executor":
        catalog._executors["pubmed.search"] = lambda *_args, **_kwargs: pytest.fail("replacement entered")
    elif mutation == "descriptor":
        object.__setattr__(catalog.resolve("pubmed.search"), "description", "changed")
    elif mutation == "closure":
        cell = catalog._executors["pubmed.search"].__closure__[0]
        cell.cell_contents = "ChangedCapturedControl"
    else:
        catalog._executors["openalex.search_works"].__kwdefaults__["_fetcher"] = lambda *_args, **_kwargs: None
    for action in (registry.to_dict, lambda: registry.execute(request, approve_source_dispatch=True)):
        with pytest.raises(ArgumentError, match="binding changed"):
            action()


def test_changed_component_bytes_are_refused_before_schema_inspection_or_executor_entry(monkeypatch):
    registry = reviewed_workflow_provider_registry(_all_catalog())
    request = registry.prepare("pubmed.search", {})
    original = registry_module._component_digest
    monkeypatch.setattr(registry_module, "_component_digest", lambda name: "0" * 64 if name == "reviewed_pubmed_retrieval" else original(name))
    for action in (registry.to_dict, lambda: registry.inspect_request("pubmed.search", {}), lambda: registry.execute(request, approve_source_dispatch=True)):
        with pytest.raises(ArgumentError, match="component source changed"):
            action()


def test_normalized_defaults_are_bound_before_a_request_can_be_approved():
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    request = registry.prepare("pubmed.search", {})
    document = request.to_dict()
    assert document["arguments"] == {"specialty_lanes": ["glioma"], "per_specialty_limit": 10, "timeout_seconds": 30}
    assert document["arguments_digest"] == content_digest(document["arguments"])
    assert document == registry.prepare("pubmed.search", document["arguments"]).to_dict()
    inspection = registry.inspect_request("pubmed.search", {})
    assert inspection["request"] == document
    assert inspection["executor_entered"] is False and inspection["source_requests"] == 0
    assert inspection["remote_response_behavior"] == "unmeasured"
    document["arguments"]["per_specialty_limit"] = 1
    assert request.to_dict()["arguments"]["per_specialty_limit"] == 10


@pytest.mark.parametrize("approval", [False, 1, "true", None])
def test_prepared_request_grants_no_source_authority(approval, monkeypatch):
    registry = reviewed_workflow_provider_registry(_all_catalog())
    request = registry.prepare("openalex.search_works", {})
    monkeypatch.setattr(research, "_builtin_fetch", lambda *_args, **_kwargs: pytest.fail("unapproved dispatch"))
    with pytest.raises(ArgumentError, match="literal source approval"):
        registry.execute(request, approve_source_dispatch=approval)


@pytest.mark.parametrize("operation_id,arguments", [
    ("pubmed.search", {"per_specialty_limit": 51}),
    ("pubmed.search", {"url": "https://unreviewed.invalid"}),
    ("pubmed.search", {"api_key": "SECRET_DO_NOT_PRINT"}),
    ("unknown.reader", {}),
    ("uniprot.protein_annotation", {}),
])
def test_unreviewed_disabled_or_invalid_requests_never_enter_an_executor(operation_id, arguments):
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    with pytest.raises(ValueError):
        registry.prepare(operation_id, arguments)


def _fixture_registry(monkeypatch, *, failure=False):
    """A real adapter and normalizer with a replaced foreign module global remains a fixture.

    This deliberately demonstrates why a bound default executor and builtin_https receipt are
    insufficient to authenticate remote network traffic.
    """
    calls = []
    registry = reviewed_workflow_provider_registry(_all_catalog())
    def fetch(url, **kwargs):
        calls.append((url, kwargs))
        if failure:
            raise RuntimeError("SECRET_DO_NOT_PRINT bearer private text")
        return research.ResearchSourceHttpResponse(200, b'{"meta":{"count":0},"results":[]}', url, "application/json")
    monkeypatch.setattr(research, "_builtin_fetch", fetch)
    monkeypatch.setattr(research, "_pace", lambda *_args: None)
    return registry, calls


def test_real_normalizer_fixture_execution_keeps_unknown_http_count_and_never_upgrades_live_coverage(monkeypatch):
    registry, calls = _fixture_registry(monkeypatch)
    request = registry.prepare("openalex.search_works", {"query": "glioma", "limit": 1})
    execution = registry.execute(request, approve_source_dispatch=True, retrieved_at="2026-10-27T12:00:00Z")
    assert len(calls) == 1
    assert execution.result.value["returned_count"] == 0
    assert execution.result.receipt["network"] == "builtin_https"
    observation = execution.observation()
    assert observation["outcome"] == "schema_validated_result"
    assert observation["catalogue_execute_invocation_count"] == 1
    assert observation["actual_http_request_count"] == {"state": "not_independently_observed", "value": None}
    assert observation["supports_live_provider_coverage"] is False
    assert observation["network_authenticity"] == "unassessed"
    assert "records" not in json.dumps(observation)
    assert registry.to_dict()["counts"]["independently_live_verified_providers"]["value"] is None
    before = len(calls)
    verified = registry.verify_observation(observation, request, result=execution.result)
    assert len(calls) == before
    assert verified["source_calls"] == verified["model_calls"] == 0
    assert verified["live_acquisition_reverified"] is False


def test_failed_adapter_attempt_has_no_result_or_invented_zero_and_hides_foreign_error_text(monkeypatch):
    registry, calls = _fixture_registry(monkeypatch, failure=True)
    request = registry.prepare("openalex.search_works", {})
    with pytest.raises(WorkflowProviderDispatchError) as caught:
        registry.execute(request, approve_source_dispatch=True)
    assert len(calls) == 1
    observation = caught.value.observation()
    assert observation["outcome"] == "adapter_or_schema_failed"
    assert observation["result"] is None
    assert observation["actual_http_request_count"]["value"] is None
    assert "SECRET_DO_NOT_PRINT" not in str(caught.value) + json.dumps(observation)
    assert registry.verify_observation(observation, request)["outcome"] == "adapter_or_schema_failed"
    with pytest.raises(ArgumentError, match="must not invent"):
        registry.verify_observation(observation, request, result=WorkflowOperationResult({}, {}))


@pytest.mark.parametrize("adapter_fails", [False, True])
def test_postdispatch_binding_change_preserves_entered_attempt_and_actual_adapter_outcome(adapter_fails, monkeypatch):
    catalog = _all_catalog()
    registry = reviewed_workflow_provider_registry(catalog)
    request = registry.prepare("openalex.search_works", {})
    calls = []
    def fetch(url, **_kwargs):
        calls.append(url)
        catalog._executors["pubmed.search"] = lambda *_args, **_kwargs: None
        if adapter_fails:
            raise RuntimeError("SECRET_DO_NOT_PRINT mutation and failure")
        return research.ResearchSourceHttpResponse(200, b'{"meta":{"count":0},"results":[]}', url, "application/json")
    monkeypatch.setattr(research, "_builtin_fetch", fetch)
    monkeypatch.setattr(research, "_pace", lambda *_args: None)
    with pytest.raises(WorkflowProviderDispatchError) as caught:
        registry.execute(request, approve_source_dispatch=True)
    observation = caught.value.observation()
    assert len(calls) == 1
    assert observation["catalogue_execute_invocation_count"] == 1
    assert observation["outcome"] == "binding_invalidated_after_dispatch"
    assert observation["adapter_outcome"] == ("adapter_or_schema_failed" if adapter_fails else "schema_validated_result")
    assert observation["result_admitted"] is False
    assert (observation["result"] is None) == adapter_fails
    if not adapter_fails:
        assert len(observation["result"]["value_digest"]) == len(observation["result"]["receipt_digest"]) == 64
    assert "SECRET_DO_NOT_PRINT" not in str(caught.value) + json.dumps(observation)
    assert observation["actual_http_request_count"]["value"] is None
    assert observation["observation_digest"] == content_digest({key: value for key, value in observation.items() if key != "observation_digest"})


@pytest.mark.parametrize("corruption", ["non_operation", "unserializable_descriptor"])
def test_uninspectable_postdispatch_binding_preserves_attempt_observation(corruption, monkeypatch):
    catalog = _all_catalog()
    registry = reviewed_workflow_provider_registry(catalog)
    request = registry.prepare("openalex.search_works", {})
    calls = []
    def fetch(url, **_kwargs):
        calls.append(url)
        if corruption == "non_operation":
            catalog._operations["pubmed.search"] = object()
        else:
            object.__setattr__(catalog.resolve("pubmed.search"), "_input_json", object())
        return research.ResearchSourceHttpResponse(200, b'{"meta":{"count":0},"results":[]}', url, "application/json")
    monkeypatch.setattr(research, "_builtin_fetch", fetch)
    monkeypatch.setattr(research, "_pace", lambda *_args: None)
    with pytest.raises(WorkflowProviderDispatchError) as caught:
        registry.execute(request, approve_source_dispatch=True)
    assert len(calls) == 1
    assert caught.value.observation()["outcome"] == "binding_invalidated_after_dispatch"
    assert caught.value.observation()["adapter_outcome"] == "schema_validated_result"
    assert caught.value.observation()["catalogue_execute_invocation_count"] == 1
    with pytest.raises(ArgumentError, match="binding inspection failed"):
        registry.to_dict()


@pytest.mark.parametrize("edit", ["request", "result", "receipt", "live", "boolean_count", "unknown_field"])
def test_rehashed_observations_cannot_change_request_result_or_unknown_measurement_states(edit, monkeypatch):
    registry, _calls = _fixture_registry(monkeypatch)
    request = registry.prepare("openalex.search_works", {})
    execution = registry.execute(request, approve_source_dispatch=True)
    observation = execution.observation()
    if edit == "request":
        observation["request_digest"] = "0" * 64
    elif edit in {"result", "receipt"}:
        observation["result"]["value_digest" if edit == "result" else "receipt_digest"] = "0" * 64
    elif edit == "live":
        observation["supports_live_provider_coverage"] = True
    elif edit == "boolean_count":
        observation["catalogue_execute_invocation_count"] = True
    else:
        observation["claimed_provider_count"] = 999
    observation["observation_digest"] = content_digest({key: value for key, value in observation.items() if key != "observation_digest"})
    with pytest.raises(ArgumentError, match="inconsistent"):
        registry.verify_observation(observation, request, result=execution.result)


def test_changed_retained_native_value_is_detected_without_reacquisition(monkeypatch):
    registry, calls = _fixture_registry(monkeypatch)
    request = registry.prepare("openalex.search_works", {})
    execution = registry.execute(request, approve_source_dispatch=True)
    execution.result.value["reported_total_count"] = 1
    with pytest.raises(ArgumentError, match="inconsistent"):
        registry.verify_observation(execution.observation(), request, result=execution.result)
    assert len(calls) == 1


def test_rehashed_prepared_request_cannot_cross_catalogue_or_contract_binding(monkeypatch):
    registry, calls = _fixture_registry(monkeypatch)
    request = registry.prepare("openalex.search_works", {})
    changed = request.to_dict()
    changed["registry_binding_digest"] = "0" * 64
    changed["request_digest"] = content_digest({key: value for key, value in changed.items() if key != "request_digest"})
    with pytest.raises(ArgumentError, match="exact current binding"):
        registry.execute(WorkflowProviderRequest(canonical_json(changed)), approve_source_dispatch=True)
    assert calls == []


@pytest.mark.parametrize("document", ["[]", "{", '{"arguments":NaN}', " " * 2_100_001],
                         ids=["array", "malformed", "nonfinite", "oversize"])
def test_manually_constructed_request_cannot_bypass_bounded_finite_object_parsing(document):
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    with pytest.raises(ArgumentError):
        registry.execute(WorkflowProviderRequest(document), approve_source_dispatch=True)


def test_external_acquisition_replay_and_fixture_references_never_promote_live_coverage():
    references = tuple(WorkflowProviderEvidenceReference("pubmed.search", kind, "report." + kind, "1" * 64)
                       for kind in ("acquisition", "offline_replay", "fixture", "failed_attempt", "unjudged_attempt"))
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog(), evidence_references=references)
    document = registry.to_dict()
    assert document["counts"]["external_evidence_references"] == 5
    assert {value["evidence_kind"] for value in document["evidence_references"]} == {"acquisition", "offline_replay", "fixture", "failed_attempt", "unjudged_attempt"}
    assert all(value["supports_live_coverage"] is False and value["artifact_opened"] is False and value["verification_state"] == "unverified_external_reference" for value in document["evidence_references"])
    assert document["counts"]["independently_live_verified_providers"] == {"state": "not_established_by_registry", "value": None}
    assert _reader(document, "pubmed.search")["live_observation"]["value"] is None


def test_duplicate_or_unknown_external_evidence_references_are_refused():
    reference = WorkflowProviderEvidenceReference("pubmed.search", "acquisition", "same.report", "1" * 64)
    with pytest.raises(ArgumentError, match="duplicated"):
        reviewed_workflow_provider_registry(builtin_workflow_operation_catalog(), evidence_references=(reference, reference))
    with pytest.raises(ArgumentError, match="unknown reader"):
        reviewed_workflow_provider_registry(builtin_workflow_operation_catalog(), evidence_references=(replace(reference, operation_id="fake.reader"),))


@pytest.mark.parametrize("origin", ["http://api.example.org", "https://user:secret@api.example.org", "https://api.example.org:444", "https://api.example.org:bad", "https://api.example.org/path", "https://api.example.org?token=secret", "https://api.example.org#fragment", "https://127.0.0.1", "https://[::1]"])
def test_service_identity_cannot_smuggle_credentials_paths_or_nondefault_authorities(origin):
    with pytest.raises(ArgumentError):
        WorkflowProviderIdentity("service", "Service", (origin,), "Reviewed service identity fixture")


def test_identity_reader_and_contract_deduplication_are_validated_before_inspection():
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    document = json.loads(registry._manifest)
    provider = WorkflowProviderIdentity("ncbi_eutils", "NCBI", ("https://eutils.ncbi.nlm.nih.gov",), "One service grouping")
    with pytest.raises(ArgumentError, match="duplicated"):
        _reconstruct(registry, providers=(provider, provider))
    raw = document["readers"][0]
    reader = WorkflowReaderContract(raw["operation_id"], "unknown", raw["operation"], raw["component_sha256"], raw["review_document"])
    with pytest.raises(ArgumentError, match="unknown service"):
        _reconstruct(registry, readers=(reader,))
    with pytest.raises(ArgumentError, match="exactly"):
        _reconstruct(registry, readers=())
    bad = deepcopy(raw["operation"])
    bad["read_only"] = False
    with pytest.raises(ArgumentError, match="read-only"):
        WorkflowReaderContract(raw["operation_id"], raw["provider_id"], bad, raw["component_sha256"], raw["review_document"])


def test_conflicting_shared_component_digests_cannot_be_hidden_by_a_later_reader():
    registry = reviewed_workflow_provider_registry(builtin_workflow_operation_catalog())
    records = json.loads(registry._manifest)["readers"]
    readers = []
    for index, raw in enumerate(records):
        components = raw["component_sha256"] | ({"authoring": "0" * 64} if index == 0 else {})
        readers.append(WorkflowReaderContract(raw["operation_id"], raw["provider_id"], raw["operation"], components, raw["review_document"]))
    with pytest.raises(ArgumentError, match="conflicting component"):
        _reconstruct(registry, readers=tuple(readers))

