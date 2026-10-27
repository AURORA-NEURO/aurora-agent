from __future__ import annotations

from dataclasses import replace
from importlib import import_module
import json
from urllib.parse import parse_qs, urlsplit

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.tooling import ToolSchemaError
from prism_sdk.workflow_integrations import (
    WorkflowOperation,
    WorkflowOperationCatalog,
    WorkflowOperationResult,
    builtin_workflow_operation_catalog,
)


_AT = "2026-10-26T12:00:00Z"
_OP_IDS = {
    "pubmed.search", "europe_pmc.search", "clinical_trials.search", "gdc.projects",
    "cbioportal.studies", "ncbi_gene.summary", "open_targets.associations",
    "gwas_catalog.associations",
}


def _operation(name: str = "local.inspect", integration: str = "local", **overrides) -> WorkflowOperation:
    fields = dict(
        operation_id=name, integration_id=integration, description="Inspect local metadata",
        input_schema={"type": "object", "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 5, "default": 2}}, "additionalProperties": False},
        output_schema={"type": "object", "required": ["count"], "properties": {"count": {"type": "integer"}}, "additionalProperties": False},
        implementation="local",
    )
    fields.update(overrides)
    return WorkflowOperation(**fields)


def _local_executor(arguments, *, approve_source_dispatch, retrieved_at):
    return WorkflowOperationResult({"count": arguments["limit"]}, {"implementation": "local", "executed_at": retrieved_at})


def test_builtin_catalogue_has_eight_concrete_sources_and_deterministic_identity_without_dispatch(monkeypatch):
    import socket

    monkeypatch.setattr(socket, "create_connection", lambda *_args, **_kwargs: pytest.fail("catalogue construction opened a socket"))
    first = builtin_workflow_operation_catalog()
    second = builtin_workflow_operation_catalog()
    assert {operation.operation_id for operation in first.operations()} == _OP_IDS
    assert first.to_dict() == second.to_dict()
    assert first.to_dict()["operation_count"] == first.to_dict()["integration_count"] == 8
    assert all(operation.implementation == "live_adapter" for operation in first.operations())
    assert len(first.catalog_digest) == 64
    assert "1024" not in json.dumps(first.to_dict())
    for operation in first.operations():
        arguments = first.validate_arguments(operation.operation_id, {})
        assert arguments
        with pytest.raises(ArgumentError, match="approval"):
            first.execute(operation.operation_id, arguments)


def test_catalogue_defaults_validate_unknown_fields_and_preserve_anchored_schema_identity():
    operation = _operation()
    catalog = WorkflowOperationCatalog((operation,), {operation.operation_id: _local_executor})
    digest = catalog.catalog_digest
    operation.input_schema["properties"]["limit"]["maximum"] = 99
    assert catalog.catalog_digest == digest
    assert catalog.validate_arguments(operation.operation_id, {}) == {"limit": 2}
    for arguments in ({"limit": 6}, {"limit": True}, {"limit": "3"}, {"arbitrary": 1}):
        with pytest.raises(ToolSchemaError):
            catalog.validate_arguments(operation.operation_id, arguments)
    with pytest.raises(ArgumentError):
        catalog.validate_arguments(operation.operation_id, {"api_key": "never-display-this"})
    assert catalog.execute(operation.operation_id, {}, retrieved_at=_AT).value == {"count": 2}


def test_unsupported_optional_schema_branches_and_unchecked_patterns_fail_closed():
    with pytest.raises(ArgumentError, match="unsupported"):
        _operation(input_schema={"type": "object", "properties": {"unused": {"type": "string", "$ref": "hidden"}}})
    with pytest.raises(ArgumentError, match="format"):
        _operation(input_schema={"type": "object", "properties": {"unused": {"type": "string", "format": "uri"}}})
    operation = _operation(input_schema={"type": "object", "properties": {"text": {"type": "string", "pattern": "["}}})
    catalog = WorkflowOperationCatalog((operation,), {operation.operation_id: _local_executor})
    with pytest.raises(ToolSchemaError, match="unchecked"):
        catalog.validate_arguments(operation.operation_id, {"text": "abc"})


def test_registration_requires_executors_refuses_duplicates_and_scales_without_generated_entries():
    catalog = WorkflowOperationCatalog()
    initial_digest = catalog.catalog_digest
    for index in range(2_048):
        catalog.register(_operation(f"fixture.operation_{index}", f"fixture.integration_{index % 512}"), _local_executor)
    assert catalog.to_dict()["operation_count"] == 2_048
    assert catalog.to_dict()["integration_count"] == 512
    assert catalog.catalog_digest != initial_digest
    assert catalog.execute("fixture.operation_2047", {"limit": 3}).value == {"count": 3}
    with pytest.raises(ArgumentError, match="already registered"):
        catalog.register(_operation("fixture.operation_2047"), _local_executor)
    with pytest.raises(ArgumentError, match="exactly one executor"):
        WorkflowOperationCatalog((_operation(),))
    with pytest.raises(ArgumentError):
        catalog.register(_operation("no.executor"), None)
    with pytest.raises(ArgumentError, match="not registered"):
        catalog.resolve("service.unsupported")


def test_live_and_mutating_local_operations_require_literal_approval_before_executor_entry():
    calls = []

    def execute(arguments, **kwargs):
        calls.append((arguments, kwargs))
        return WorkflowOperationResult({"count": 1}, {})

    catalog = WorkflowOperationCatalog()
    for operation in (_operation("live.read", implementation="live_adapter"), _operation("local.write", read_only=False)):
        catalog.register(operation, execute)
        for approval in (False, 1, "true", None):
            with pytest.raises(ArgumentError):
                catalog.execute(operation.operation_id, {}, approve_source_dispatch=approval)
        assert calls == []
    assert catalog.execute("live.read", {}, approve_source_dispatch=True).value == {"count": 1}
    assert len(calls) == 1


def test_output_schema_and_result_type_are_checked_after_execution():
    catalog = WorkflowOperationCatalog()
    catalog.register(_operation("bad.shape"), lambda *_args, **_kwargs: WorkflowOperationResult({"count": "unmeasured"}, {}))
    catalog.register(_operation("bad.type"), lambda *_args, **_kwargs: {"count": 3})
    with pytest.raises(ToolSchemaError):
        catalog.execute("bad.shape", {})
    with pytest.raises(ArgumentError, match="WorkflowOperationResult"):
        catalog.execute("bad.type", {})


def _fixture_fetch(source, calls):
    def fetch(url, *args):
        calls.append((url, args))
        if source == "pubmed":
            if "esearch.fcgi" in url:
                return {"esearchresult": {"idlist": ["20000"]}}
            if "esummary.fcgi" in url:
                return {"result": {"20000": {"title": "Bounded glioma evidence fixture", "fulljournalname": "Research metadata", "pubdate": "2026 Jan 02"}}}
            return b"<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>20000</PMID><Article><Abstract><AbstractText>Fixture abstract for testing citation preservation.</AbstractText></Abstract></Article></MedlineCitation></PubmedArticle></PubmedArticleSet>"
        if source == "europe_pmc":
            return {"version": "6.9", "hitCount": 0, "request": {"queryString": "fixed", "resultType": "lite"}, "resultList": {"result": []}}
        if source == "clinical_trials":
            return {"totalCount": 3, "nextPageToken": "fixture-next", "studies": [{"protocolSection": {"identificationModule": {"nctId": "NCT01234567", "briefTitle": "Fixture registry record"}, "statusModule": {"overallStatus": "RECRUITING", "lastUpdatePostDateStruct": {"date": "2026-10-01"}}, "designModule": {"phases": ["PHASE2"], "studyType": "INTERVENTIONAL", "enrollmentInfo": {"count": 42}}}}]}
        if source == "gdc":
            project_id = urlsplit(url).path.rsplit("/", 1)[-1]
            return {"data": {"project_id": project_id, "name": "Aggregate glioma fixture", "disease_type": ["Gliomas"], "primary_site": ["Brain"], "state": "open", "released": True, "summary": {"case_count": 615, "file_count": 2405, "data_categories": [{"data_category": "Clinical", "case_count": 615, "file_count": 5}]}}, "warnings": {}}
        if source == "cbioportal":
            if "molecular-profiles" in url:
                return []
            return {"studyId": "gbm_tcga", "cancerTypeId": "gbm", "name": "Public glioma fixture", "description": "Catalogue only", "publicStudy": True, "pmid": "22824167", "allSampleCount": 619, "referenceGenome": "hg19", "importDate": "2023-01-02"}
        if source == "ncbi_gene":
            return {"header": {"type": "esummary", "version": "0.3"}, "result": {"uids": ["3417"], "3417": {"uid": "3417", "name": "IDH1", "description": "isocitrate dehydrogenase 1", "chromosome": "2", "maplocation": "2q34", "otheraliases": "IDP", "organism": {"taxid": 9606, "scientificname": "Homo sapiens"}}}}
        if source == "open_targets":
            return {"data": {"disease": {"id": "MONDO_0018177", "name": "glioblastoma", "associatedTargets": {"count": 0, "rows": []}}}}
        if source == "gwas_catalog":
            query = parse_qs(urlsplit(url).query)
            return {"_links": {"self": {"href": url}}, "page": {"size": int(query["size"][0]), "totalElements": 0, "totalPages": 0, "number": 0}}
        raise AssertionError(source)
    return fetch


@pytest.mark.parametrize("operation_id,module_name,adapter_name,arguments,array_key,request_count", [
    ("pubmed.search", "reviewed_pubmed_retrieval", "ReviewedPubMedRetrievalAdapter", {"per_specialty_limit": 1}, "records", 3),
    ("europe_pmc.search", "reviewed_europe_pmc_retrieval", "ReviewedEuropePmcRetrievalAdapter", {}, "publications", 1),
    ("clinical_trials.search", "reviewed_clinical_trials_retrieval", "ReviewedClinicalTrialsRetrievalAdapter", {"page_size": 1, "max_pages": 1}, "trials", 1),
    ("gdc.projects", "reviewed_gdc_project_retrieval", "ReviewedGdcRetrievalAdapter", {}, "projects", 1),
    ("cbioportal.studies", "reviewed_cbioportal_study_retrieval", "ReviewedCBioPortalRetrievalAdapter", {}, "studies", 2),
    ("ncbi_gene.summary", "reviewed_ncbi_gene_retrieval", "ReviewedNcbiGeneRetrievalAdapter", {"gene_symbols": ["IDH1"]}, "genes", 1),
    ("open_targets.associations", "reviewed_open_targets_retrieval", "ReviewedOpenTargetsRetrievalAdapter", {}, "associations", 1),
    ("gwas_catalog.associations", "reviewed_gwas_catalog_retrieval", "ReviewedGwasCatalogRetrievalAdapter", {"lanes": ["gbm"]}, "lanes", 1),
])
def test_builtin_wrappers_execute_actual_reviewed_adapters_through_fixture_transports(
    monkeypatch, operation_id, module_name, adapter_name, arguments, array_key, request_count,
):
    """Exercise real plan/parser/receipt code while keeping every network call fixture-owned."""
    source = operation_id.split(".")[0]
    calls = []
    module = import_module(f"prism_sdk.{module_name}")
    adapter_class = getattr(module, adapter_name)
    original_init = adapter_class.__init__
    fetcher = _fixture_fetch(source, calls)

    def fixture_init(self, config, *, fetch=None):
        assert fetch is None
        fixture_config = replace(config, transport_id=f"fixture.workflow.{source}", transport_config_digest=content_digest({"fixture": source}))
        original_init(self, fixture_config, fetch=fetcher)

    monkeypatch.setattr(adapter_class, "__init__", fixture_init)
    catalog = builtin_workflow_operation_catalog()
    with pytest.raises(ArgumentError, match="approval"):
        catalog.execute(operation_id, arguments, retrieved_at=_AT)
    assert calls == []
    result = catalog.execute(operation_id, arguments, approve_source_dispatch=True, retrieved_at=_AT)
    assert isinstance(result.value[array_key], list)
    assert result.value["sources"]
    assert result.receipt["request_count"] == request_count
    assert len(calls) == request_count
    assert len(result.receipt["receipt_digest"]) == 64
    assert "Fixture registry record" not in json.dumps(result.receipt)
    if source == "clinical_trials":
        assert result.value["truncated"] is True
        assert result.value["omitted_record_count"] == 2
        assert result.value["trials"][0]["nct_id"] == "NCT01234567"
    if source == "pubmed":
        assert result.value["records"][0]["pmid"] == "20000"


@pytest.mark.parametrize("operation_id,arguments", [
    ("pubmed.search", {"specialty_lanes": ["unreviewed"]}),
    ("pubmed.search", {"timeout_seconds": 0.5}),
    ("clinical_trials.search", {"condition_lanes": ["glioma", "glioma"]}),
    ("gdc.projects", {"project_ids": ["TCGA-BRCA"]}),
    ("cbioportal.studies", {"url": "https://unreviewed.invalid"}),
    ("ncbi_gene.summary", {"gene_symbols": ["UNREVIEWED"]}),
    ("open_targets.associations", {"lanes": ["glioma"]}),
    ("gwas_catalog.associations", {"page_size": 51}),
])
def test_source_argument_contracts_reject_scope_widening_before_dispatch(operation_id, arguments):
    with pytest.raises((ArgumentError, ToolSchemaError)):
        builtin_workflow_operation_catalog().execute(operation_id, arguments, approve_source_dispatch=True)
