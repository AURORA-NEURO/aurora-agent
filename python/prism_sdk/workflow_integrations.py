"""Executable, schema-checked operation catalogue for the workflow builder.

This repository-defined SDK layer composes the existing reviewed retrieval adapters. Eight
public-source integrations have concrete transports; catalogue capacity is not integration
coverage. Registration always requires an executor, and never discovers a service, obtains
credentials, authorizes dispatch, or makes a scientific or clinical claim. The original source
bundles stay transient and their existing receipts retain omissions and source limitations.

OpenAPI import, OAuth, deployment scheduling and independent evidence-quality review are not
implemented here. Applications may register their own bounded executors without adding generated
or descriptor-only catalogue entries.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from importlib import import_module
import json
import re
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .tooling import ToolCatalogue, ToolDefinition, ToolSchemaError


WORKFLOW_OPERATION_SCHEMA = "aurora-workflow-operation/0.1"
WORKFLOW_OPERATION_CATALOG_SCHEMA = "aurora-workflow-operation-catalog/0.1"
MAX_WORKFLOW_OPERATIONS = 4_096
MAX_WORKFLOW_INTEGRATIONS = 1_024
MAX_WORKFLOW_ARGUMENT_BYTES = 2_000_000
MAX_WORKFLOW_RESULT_BYTES = 32_000_000
_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")
_SCHEMA_KEYWORDS = frozenset({
    "$comment", "$id", "$schema", "default", "description", "examples", "title",
    "additionalProperties", "allOf", "anyOf", "const", "enum", "exclusiveMaximum",
    "exclusiveMinimum", "format", "items", "maxItems", "maxLength", "maxProperties",
    "maximum", "minItems", "minLength", "minProperties", "minimum", "not", "oneOf",
    "pattern", "properties", "required", "type", "uniqueItems",
})


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not _IDENTIFIER.fullmatch(value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _json_object(name: str, value: Any, maximum: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a JSON object")
    try:
        encoded = canonical_json(dict(value))
    except (TypeError, ValueError, RecursionError) as error:
        raise ArgumentError(f"{name} must contain bounded canonical JSON") from error
    if len(encoded.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return json.loads(encoded)


def _check_schema(schema: Any, depth: int = 0) -> None:
    """Reject unsupported branches even when an optional input never exercises them."""
    if depth > 32:
        raise ArgumentError("workflow operation schema is too deeply nested")
    if isinstance(schema, bool):
        return
    if not isinstance(schema, Mapping):
        raise ArgumentError("workflow operation schema branches must be objects or booleans")
    if set(schema) - _SCHEMA_KEYWORDS:
        raise ArgumentError("workflow operation schema contains unsupported keywords")
    properties = schema.get("properties", {})
    if not isinstance(properties, Mapping):
        raise ArgumentError("workflow operation schema properties must be an object")
    for child in properties.values():
        _check_schema(child, depth + 1)
    for name in ("items", "additionalProperties", "not"):
        if name in schema:
            _check_schema(schema[name], depth + 1)
    for name in ("allOf", "anyOf", "oneOf"):
        if name in schema:
            branches = schema[name]
            if not isinstance(branches, list) or not branches:
                raise ArgumentError("workflow operation schema alternatives must be non-empty arrays")
            for child in branches:
                _check_schema(child, depth + 1)
    if "format" in schema:
        # ToolCatalogue retains format as annotation rather than asserting its semantics.
        raise ArgumentError("workflow operation schema format constraints are not implemented")


@dataclass(frozen=True)
class WorkflowOperation:
    """A reviewed operation descriptor, separate from its caller-owned executor."""

    operation_id: str
    description: str
    input_schema: Mapping[str, Any]
    output_schema: Mapping[str, Any]
    integration_id: str
    auth_mode: str = "none"
    read_only: bool = True
    implementation: str = "live_adapter"
    _input_json: str = field(init=False, repr=False, compare=False)
    _output_json: str = field(init=False, repr=False, compare=False)

    def __post_init__(self) -> None:
        _identifier("workflow operation_id", self.operation_id)
        _identifier("workflow integration_id", self.integration_id)
        _identifier("workflow auth_mode", self.auth_mode)
        if not isinstance(self.description, str) or not self.description.strip() or len(self.description.encode("utf-8")) > 8_192:
            raise ArgumentError("workflow operation description is outside its bound")
        if type(self.read_only) is not bool:
            raise ArgumentError("workflow operation read_only must be a boolean")
        if self.implementation not in {"live_adapter", "local", "caller_adapter"}:
            raise ArgumentError("workflow operation implementation must identify an executable adapter")
        for name in ("input_schema", "output_schema"):
            safe = _json_object(name, getattr(self, name), 1_000_000)
            if safe.get("type") != "object":
                raise ArgumentError("workflow operation schemas must describe object roots")
            _check_schema(safe)
            object.__setattr__(self, name, safe)
            object.__setattr__(self, "_input_json" if name == "input_schema" else "_output_json", canonical_json(safe))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": WORKFLOW_OPERATION_SCHEMA,
            "operation_id": self.operation_id,
            "description": self.description,
            "input_schema": json.loads(self._input_json),
            "output_schema": json.loads(self._output_json),
            "integration_id": self.integration_id,
            "auth_mode": self.auth_mode,
            "read_only": self.read_only,
            "implementation": self.implementation,
        }


@dataclass(frozen=True)
class WorkflowOperationResult:
    """Transient source value and its existing metadata-only acquisition receipt."""

    value: Mapping[str, Any] = field(repr=False)
    receipt: Mapping[str, Any]

    def __post_init__(self) -> None:
        object.__setattr__(self, "value", _json_object("workflow operation value", self.value, MAX_WORKFLOW_RESULT_BYTES))
        object.__setattr__(self, "receipt", _json_object("workflow operation receipt", self.receipt, 2_000_000))


WorkflowOperationExecutor = Callable[..., WorkflowOperationResult]


class WorkflowOperationCatalog:
    """Bounded registry of concrete executors; capacity never creates support entries."""

    def __init__(
        self,
        operations: Sequence[WorkflowOperation] = (),
        executors: Mapping[str, WorkflowOperationExecutor] | None = None,
    ) -> None:
        if isinstance(operations, (str, bytes)) or not isinstance(operations, Sequence):
            raise ArgumentError("workflow operations must be a sequence")
        supplied = {} if executors is None else dict(executors)
        if any(not isinstance(operation, WorkflowOperation) for operation in operations):
            raise ArgumentError("workflow operations must contain WorkflowOperation values")
        if {operation.operation_id for operation in operations} != set(supplied):
            raise ArgumentError("every workflow operation requires exactly one executor")
        self._operations: dict[str, WorkflowOperation] = {}
        self._executors: dict[str, WorkflowOperationExecutor] = {}
        for operation in operations:
            self.register(operation, supplied[operation.operation_id])

    def register(self, operation: WorkflowOperation, executor: WorkflowOperationExecutor) -> WorkflowOperation:
        if not isinstance(operation, WorkflowOperation) or not callable(executor):
            raise ArgumentError("workflow registration requires an operation and a callable executor")
        if operation.operation_id in self._operations:
            raise ArgumentError("workflow operation is already registered")
        if len(self._operations) >= MAX_WORKFLOW_OPERATIONS:
            raise ArgumentError("workflow operation catalogue is full")
        integrations = {item.integration_id for item in self._operations.values()}
        if operation.integration_id not in integrations and len(integrations) >= MAX_WORKFLOW_INTEGRATIONS:
            raise ArgumentError("workflow integration catalogue is full")
        self._operations[operation.operation_id] = operation
        self._executors[operation.operation_id] = executor
        return operation

    def operations(self) -> tuple[WorkflowOperation, ...]:
        return tuple(self._operations[name] for name in sorted(self._operations))

    def subset(self, operation_ids: Sequence[str]) -> "WorkflowOperationCatalog":
        """Retain exact executors for an explicit subset; discovery grants no authority."""
        if isinstance(operation_ids, (str, bytes)) or not isinstance(operation_ids, Sequence):
            raise ArgumentError("workflow catalogue subset requires an explicit sequence")
        if (len(operation_ids) > MAX_WORKFLOW_OPERATIONS or any(type(name) is not str for name in operation_ids)
                or len(set(operation_ids)) != len(operation_ids)):
            raise ArgumentError("workflow catalogue subset is duplicated or exceeds its bound")
        operations = tuple(self.resolve(name) for name in operation_ids)
        return WorkflowOperationCatalog(operations, {item.operation_id: self._executors[item.operation_id] for item in operations})

    def extended(self, extension: "WorkflowOperationCatalog") -> "WorkflowOperationCatalog":
        """Combine atomically, refusing descriptor conflicts and preserving existing bindings.

        Matching descriptors never replace a caller-owned executor or establish its conformance.
        Both input catalogues remain unchanged if validation or capacity checks fail.
        """
        if type(extension) is not WorkflowOperationCatalog:
            raise ArgumentError("workflow catalogue extension requires a concrete catalogue")
        operations, executors = dict(self._operations), dict(self._executors)
        for operation in extension.operations():
            name = operation.operation_id
            if name in operations:
                if canonical_json(operations[name].to_dict()) != canonical_json(operation.to_dict()):
                    raise ArgumentError("workflow catalogue extension conflicts with an existing descriptor")
            else:
                operations[name], executors[name] = operation, extension._executors[name]
        return WorkflowOperationCatalog(tuple(operations.values()), executors)

    def resolve(self, operation_id: str) -> WorkflowOperation:
        _identifier("workflow operation_id", operation_id)
        try:
            return self._operations[operation_id]
        except KeyError as error:
            raise ArgumentError("workflow operation is not registered") from error

    @property
    def catalog_digest(self) -> str:
        return content_digest({"schema": WORKFLOW_OPERATION_CATALOG_SCHEMA, "operations": [item.to_dict() for item in self.operations()]})

    def to_dict(self) -> dict[str, Any]:
        operations = self.operations()
        return {
            "schema": WORKFLOW_OPERATION_CATALOG_SCHEMA,
            "operations": [item.to_dict() for item in operations],
            "operation_count": len(operations),
            "integration_count": len({item.integration_id for item in operations}),
            "catalog_digest": self.catalog_digest,
            "execution": "catalogue_only;registration_does_not_authorize_dispatch",
            "limitations": ["Registered executors do not establish service availability or evidence quality", "Credentials and authorization remain application-owned"],
        }

    def validate_arguments(self, operation_id: str, arguments: Mapping[str, Any]) -> dict[str, Any]:
        operation = self.resolve(operation_id)
        safe = _json_object("workflow arguments", arguments, MAX_WORKFLOW_ARGUMENT_BYTES)
        _reject_secret_fields(safe)
        schema = json.loads(operation._input_json)
        # Defaults become part of the validated request identity before it can be dispatched.
        for name, definition in schema.get("properties", {}).items():
            if name not in safe and isinstance(definition, Mapping) and "default" in definition:
                safe[name] = json.loads(canonical_json(definition["default"]))
        _reject_secret_fields(safe)
        self._validate(operation_id, safe, schema)
        return safe

    @staticmethod
    def _validate(operation_id: str, value: Mapping[str, Any], schema: Mapping[str, Any]) -> None:
        catalogue = ToolCatalogue.from_definitions((ToolDefinition(operation_id, schema),))
        report = catalogue.validate(operation_id, value)
        report.raise_if_invalid()
        if not report.fully_checked:
            raise ToolSchemaError("workflow operation schema contains unchecked constraints")

    def execute(
        self,
        operation_id: str,
        arguments: Mapping[str, Any],
        *,
        approve_source_dispatch: bool = False,
        retrieved_at: str | None = None,
    ) -> WorkflowOperationResult:
        operation = self.resolve(operation_id)
        safe = self.validate_arguments(operation_id, arguments)
        if type(approve_source_dispatch) is not bool:
            raise ArgumentError("workflow source approval must be a literal boolean")
        if (operation.implementation != "local" or not operation.read_only) and approve_source_dispatch is not True:
            raise ArgumentError("workflow source dispatch requires explicit literal approval")
        result = self._executors[operation_id](safe, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at)
        if not isinstance(result, WorkflowOperationResult):
            raise ArgumentError("workflow executor must return WorkflowOperationResult")
        self._validate(operation_id, result.value, json.loads(operation._output_json))
        return result


def _selection(values: Sequence[str], default: Sequence[str]) -> dict[str, Any]:
    return {"type": "array", "items": {"type": "string", "enum": list(values)}, "minItems": 1, "maxItems": len(values), "uniqueItems": True, "default": list(default)}


def _integer(minimum: int, maximum: int, default: int) -> dict[str, Any]:
    return {"type": "integer", "minimum": minimum, "maximum": maximum, "default": default}


def _bundle_schema(array_names: Sequence[str]) -> dict[str, Any]:
    return {"type": "object", "properties": {"sources": {"type": "array", "items": {"type": "object"}}, "limitations": {"type": "array", "items": {"type": "string"}}, **{name: {"type": "array", "items": {"type": "object"}} for name in array_names}}, "required": ["sources", *array_names], "additionalProperties": True}


def _reviewed_executor(module_name: str, config_name: str, adapter_name: str) -> WorkflowOperationExecutor:
    def execute(arguments: Mapping[str, Any], *, approve_source_dispatch: bool, retrieved_at: str | None) -> WorkflowOperationResult:
        module = import_module(f".{module_name}", __package__)
        config_arguments = {name: tuple(value) if isinstance(value, list) else value for name, value in arguments.items()}
        config = getattr(module, config_name)(**config_arguments)
        adapter = getattr(module, adapter_name)(config)
        reviewed_plan = adapter.prepare()
        result = adapter.execute(reviewed_plan, approve_source_dispatch=approve_source_dispatch, retrieved_at=retrieved_at)
        receipt = result.receipt.to_dict() if hasattr(result.receipt, "to_dict") else result.receipt
        return WorkflowOperationResult(result.bundle, receipt)
    return execute


def builtin_workflow_operation_catalog() -> WorkflowOperationCatalog:
    """Return eight actual reviewed source adapters without opening any connection."""
    specialties = ("glioma", "cranial_base", "craniosynostosis", "encephalocele", "spina_bifida", "chiari_malformation")
    timeout_ms = _integer(100, 120_000, 30_000)
    specs = (
        ("pubmed.search", "pubmed", "Retrieve fixed neurosurgical PubMed lanes with citation metadata and bounded abstracts. Glioma includes adult diffuse glioma and glioblastoma. Keywords are not exhaustive.", "reviewed_pubmed_retrieval", "ReviewedPubMedRetrievalConfig", "ReviewedPubMedRetrievalAdapter", {"specialty_lanes": _selection(specialties, ("glioma",)), "per_specialty_limit": _integer(1, 50, 10), "timeout_seconds": {"type": "number", "minimum": 1, "maximum": 120, "default": 30}}, ("records",)),
        ("europe_pmc.search", "europe_pmc", "Retrieve bounded Europe PMC bibliographic metadata for fixed neurosurgical lanes; no abstracts or full text.", "reviewed_europe_pmc_retrieval", "ReviewedEuropePmcRetrievalConfig", "ReviewedEuropePmcRetrievalAdapter", {"lanes": _selection(specialties, ("glioma",)), "page_size": _integer(1, 100, 10), "max_pages": _integer(1, 4, 2), "timeout_ms": timeout_ms}, ("publications",)),
        ("clinical_trials.search", "clinical_trials", "Retrieve ClinicalTrials.gov glioma or glioblastoma registry metadata; no eligibility or treatment-benefit assessment.", "reviewed_clinical_trials_retrieval", "ReviewedClinicalTrialsRetrievalConfig", "ReviewedClinicalTrialsRetrievalAdapter", {"condition_lanes": _selection(("glioblastoma", "glioma"), ("glioblastoma",)), "page_size": _integer(1, 100, 25), "max_pages": _integer(1, 5, 2), "timeout_seconds": _integer(1, 120, 30)}, ("trials",)),
        ("gdc.projects", "gdc", "Retrieve fixed TCGA-GBM and TCGA-LGG NCI GDC project summaries and aggregate availability; no patient or molecular values.", "reviewed_gdc_project_retrieval", "ReviewedGdcRetrievalConfig", "ReviewedGdcRetrievalAdapter", {"project_ids": _selection(("TCGA-GBM", "TCGA-LGG"), ("TCGA-GBM",)), "timeout_ms": timeout_ms}, ("projects",)),
        ("cbioportal.studies", "cbioportal", "Retrieve public TCGA GBM/LGG study and molecular-profile catalogue metadata; no patient, sample, or assay values.", "reviewed_cbioportal_study_retrieval", "ReviewedCBioPortalRetrievalConfig", "ReviewedCBioPortalRetrievalAdapter", {"study_ids": _selection(("gbm_tcga", "lgg_tcga"), ("gbm_tcga",)), "timeout_ms": timeout_ms}, ("studies", "molecular_profiles")),
        ("ncbi_gene.summary", "ncbi_gene", "Retrieve identity, aliases and concise metadata for twelve fixed human glioma-research gene markers; no variant interpretation.", "reviewed_ncbi_gene_retrieval", "ReviewedNcbiGeneRetrievalConfig", "ReviewedNcbiGeneRetrievalAdapter", {"gene_symbols": _selection(("IDH1", "IDH2", "MGMT", "EGFR", "TERT", "TP53", "ATRX", "NF1", "PTEN", "CDKN2A", "PDGFRA", "BRAF"), ("IDH1", "IDH2", "MGMT", "EGFR", "TERT", "TP53", "ATRX", "NF1", "PTEN", "CDKN2A", "PDGFRA", "BRAF")), "timeout_ms": timeout_ms}, ("genes",)),
        ("open_targets.associations", "open_targets", "Retrieve one bounded ranked association page for fixed glioblastoma or low-grade glioma disease IDs; scores are not causal or clinical confidence.", "reviewed_open_targets_retrieval", "ReviewedOpenTargetsRetrievalConfig", "ReviewedOpenTargetsRetrievalAdapter", {"lanes": _selection(("gbm", "lgg"), ("gbm",)), "page_size": _integer(1, 50, 50), "timeout_ms": timeout_ms}, ("associations",)),
        ("gwas_catalog.associations", "gwas_catalog", "Retrieve bounded GWAS Catalog association metadata for fixed direct glioblastoma/glioma traits; no causal or treatment inference.", "reviewed_gwas_catalog_retrieval", "ReviewedGwasCatalogRetrievalConfig", "ReviewedGwasCatalogRetrievalAdapter", {"lanes": _selection(("gbm", "glioma"), ("gbm", "glioma")), "page_size": _integer(1, 50, 20), "max_pages": _integer(1, 5, 2), "timeout_ms": timeout_ms}, ("lanes",)),
    )
    catalog = WorkflowOperationCatalog()
    for operation_id, integration_id, description, module, config, adapter, properties, output_arrays in specs:
        operation = WorkflowOperation(operation_id, description, {"type": "object", "properties": properties, "additionalProperties": False}, _bundle_schema(output_arrays), integration_id)
        catalog.register(operation, _reviewed_executor(module, config, adapter))
    return catalog


__all__ = [
    "WORKFLOW_OPERATION_SCHEMA", "WORKFLOW_OPERATION_CATALOG_SCHEMA", "MAX_WORKFLOW_OPERATIONS",
    "MAX_WORKFLOW_INTEGRATIONS", "WorkflowOperation", "WorkflowOperationResult",
    "WorkflowOperationExecutor", "WorkflowOperationCatalog", "builtin_workflow_operation_catalog",
]
