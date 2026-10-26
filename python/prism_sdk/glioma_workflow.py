"""Compose reviewed public glioma metadata retrieval into an auditable workflow.

This repository-defined deployment contract composes existing public retrieval adapters. It
does not implement an absent BioPRISM blueprint feature, interpret molecular measurements, or
authorize a clinical or laboratory action. Public human cohort summaries remain separate from
the Rust preclinical research-engine boundary. Local reconciliation counts and cites source
records; scientific validity, source independence, and applicability remain unassessed.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
import json
import re
from typing import Any

from .authoring import content_digest
from .errors import ArgumentError


GLIOMA_WORKFLOW_SCOPE = "public_glioma_research_metadata"
GLIOMA_RECONCILIATION_SCHEMA = "aurora-glioma-evidence-reconciliation/0.1"
GLIOMA_REVIEW_HANDOFF_SCHEMA = "aurora-glioma-evidence-review-handoff/0.1"
GLIOMA_RECONCILE_OPERATION = "glioma.evidence.reconcile"
GLIOMA_REVIEW_OPERATION = "glioma.evidence.review_handoff"
MAX_GLIOMA_WORKFLOW_INPUT_BYTES = 12_000_000
MAX_GLIOMA_SOURCE_RECORDS = 2_000

_SOURCES = {
    "pubmed.search": ("bioprism-neurosurgery-public-literature/0.1", "records", "pmid", "literature_citation_metadata"),
    "clinical_trials.search": ("bioprism-reviewed-clinical-trials-bundle/0.1", "trials", "nct_id", "study_registry_metadata"),
    "gdc.projects": ("bioprism-reviewed-gdc-project-bundle/0.1", "projects", "project_id", "aggregate_project_metadata"),
}
_RECORD_FIELDS = {
    "pubmed.search": {"source_id", "specialty", "pmid", "title", "journal", "publication_date", "doi", "abstract_text", "abstract_truncated", "publication_types", "mesh_terms"},
    "clinical_trials.search": {"source_id", "nct_id", "title", "overall_status", "phases", "last_update", "study_type", "enrollment_count", "intervention_names"},
    "gdc.projects": {"source_id", "lane", "project_id", "name", "disease_types", "primary_sites", "state", "released", "case_count", "file_count", "data_categories", "metadata_completeness"},
}
_SOURCE_FIELDS = {"source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"}
_LIMITATIONS = (
    "The fixed glioma retrieval lanes do not use the free-text question as a search query.",
    "Retrieved metadata is not evidence of treatment benefit, trial eligibility, or individual applicability.",
    "Literature citations, trial registrations, and project summaries are separate evidence classes; their counts are not pooled study or patient counts.",
    "Source authenticity, study quality, evidence independence, cohort overlap, and scientific conclusions require independent review.",
    "Missing values remain missing; a project or file count does not establish molecular assay availability or measurement results.",
    "No patient-level data, clinical interpretation, preclinical-engine admission, or laboratory execution is supplied by this workflow.",
)
_UNASSESSED = (
    "IDH mutation and 1p/19q co-deletion measurements",
    "MGMT promoter methylation and assay methodology",
    "H3 alterations, TERT promoter, EGFR, and CDKN2A/B measurements",
    "tumour heterogeneity, single-cell, spatial, imaging, and longitudinal evidence",
    "cohort overlap, confounding, negative results, and applicability of scientific claims",
)


def _json_copy(value: Any, name: str) -> Any:
    try:
        encoded = json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
        if len(encoded.encode("utf-8")) > MAX_GLIOMA_WORKFLOW_INPUT_BYTES:
            raise ArgumentError(f"{name} exceeds its byte bound")
        return json.loads(encoded)
    except (TypeError, ValueError, RecursionError, UnicodeError) as error:
        if isinstance(error, ArgumentError):
            raise
        raise ArgumentError(f"{name} must be bounded finite JSON") from error


def _question(value: Any) -> str:
    try:
        valid = isinstance(value, str) and bool(value.strip()) and len(value.encode("utf-8")) <= 8_000 and "\x00" not in value
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("glioma workflow question must be non-empty text within 8000 UTF-8 bytes")
    return value


def _source_names(value: Any) -> list[str]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)) or not 1 <= len(value) <= len(_SOURCES):
        raise ArgumentError("requested_sources must be a non-empty bounded source list")
    if any(not isinstance(item, str) or item not in _SOURCES for item in value) or len(set(value)) != len(value):
        raise ArgumentError("requested_sources must name unique reviewed glioma sources")
    return list(value)


def _citation_url(operation_id: str, record_id: str) -> str:
    if not isinstance(record_id, str):
        raise ArgumentError("glioma evidence record identifier must be text")
    if operation_id == "pubmed.search" and re.fullmatch(r"[1-9][0-9]{0,11}", record_id):
        return f"https://pubmed.ncbi.nlm.nih.gov/{record_id}/"
    if operation_id == "clinical_trials.search" and re.fullmatch(r"NCT[0-9]{8}", record_id):
        return f"https://clinicaltrials.gov/study/{record_id}"
    if operation_id == "gdc.projects" and record_id in {"TCGA-GBM", "TCGA-LGG"}:
        return f"https://portal.gdc.cancer.gov/projects/{record_id}"
    raise ArgumentError("glioma evidence record identifier does not match its reviewed source")


def _source_partition(operation_id: str, bundle: Mapping[str, Any]) -> dict[str, Any]:
    schema, collection, identity, evidence_class = _SOURCES[operation_id]
    if bundle.get("schema_version", bundle.get("schema")) != schema:
        raise ArgumentError("glioma evidence bundle schema does not match the source operation")
    if bundle.get("synthetic_data") is True:
        raise ArgumentError("glioma public research workflow refuses synthetic source bundles")
    rows = bundle.get(collection)
    sources = bundle.get("sources")
    if not isinstance(rows, list) or len(rows) > MAX_GLIOMA_SOURCE_RECORDS or not isinstance(sources, list) or len(sources) > 32:
        raise ArgumentError("glioma evidence source collections are missing or outside their bounds")
    source_ids: set[str] = set()
    for source in sources:
        if not isinstance(source, Mapping) or not isinstance(source.get("source_id"), str) or not source["source_id"]:
            raise ArgumentError("glioma evidence source identity is missing")
        if set(source) - _SOURCE_FIELDS:
            raise ArgumentError("glioma evidence source contains fields outside the public metadata contract")
        if source["source_id"] in source_ids:
            raise ArgumentError("glioma evidence bundle repeats a source identity")
        source_ids.add(source["source_id"])
    if "bundle_digest" in bundle:
        unsigned = {key: value for key, value in bundle.items() if key != "bundle_digest"}
        if bundle["bundle_digest"] != content_digest(unsigned):
            raise ArgumentError("glioma evidence bundle digest does not match its contents")
    citations: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    gaps: list[dict[str, str]] = []
    for row in rows:
        if not isinstance(row, Mapping) or row.get("source_id") not in source_ids:
            raise ArgumentError("glioma evidence record has no declared source")
        if set(row) - _RECORD_FIELDS[operation_id]:
            raise ArgumentError("glioma evidence record contains fields outside the public metadata contract")
        if operation_id == "gdc.projects":
            categories = row.get("data_categories", [])
            if not isinstance(categories, list) or any(not isinstance(category, Mapping) or set(category) - {"data_category", "case_count", "file_count"} for category in categories):
                raise ArgumentError("glioma project categories contain fields outside the aggregate metadata contract")
        record_id = row.get(identity)
        if not isinstance(record_id, str):
            raise ArgumentError("glioma evidence record has no source record identifier")
        citation_url = _citation_url(operation_id, record_id)
        key = (row["source_id"], record_id)
        if key in seen:
            raise ArgumentError("glioma evidence bundle repeats a source record")
        seen.add(key)
        if operation_id == "pubmed.search":
            fields = ("title", "journal", "publication_date", "doi", "publication_types", "mesh_terms", "abstract_truncated")
            missing = [field for field in ("publication_date", "doi", "abstract_text") if row.get(field) in (None, "")]
        elif operation_id == "clinical_trials.search":
            fields = ("title", "overall_status", "phases", "last_update", "study_type", "enrollment_count", "intervention_names")
            missing = [field for field in ("last_update", "study_type", "enrollment_count") if row.get(field) in (None, "")]
        else:
            fields = ("name", "disease_types", "primary_sites", "state", "released", "case_count", "file_count", "data_categories", "metadata_completeness")
            missing = [field for field in ("case_count", "file_count", "state", "released") if row.get(field) in (None, "")]
        for field in missing:
            gaps.append({"record_id": record_id, "field": field, "state": "missing"})
        citations.append({
            "operation_id": operation_id,
            "source_id": row["source_id"],
            "record_id": record_id,
            "citation_url": citation_url,
            "record_digest": content_digest(row),
            "evidence_class": evidence_class,
            "metadata": {field: deepcopy(row.get(field)) for field in fields},
            "missing_fields": sorted(missing),
            **({"abstract_available": row.get("abstract_text") not in (None, "")} if operation_id == "pubmed.search" else {}),
        })
    citations.sort(key=lambda row: (row["source_id"], row["record_id"]))
    return {
        "operation_id": operation_id,
        "status": "records_present" if rows else "empty",
        "evidence_class": evidence_class,
        "bundle_digest": content_digest(bundle),
        "declared_bundle_digest": bundle.get("bundle_digest"),
        "generated_at": bundle.get("generated_at"),
        "record_count": len(rows),
        "sources": deepcopy(sources),
        "citations": citations,
        "metadata_gaps": sorted(gaps, key=lambda row: (row["record_id"], row["field"])),
        "source_completeness": {key: deepcopy(bundle.get(key)) for key in ("truncated", "reported_total_count", "omitted_record_count", "completeness")},
        "source_limitations": deepcopy(bundle.get("limitations", [])),
    }


def reconcile_glioma_evidence(*, question: str, requested_sources: Sequence[str], sources: Sequence[Mapping[str, Any]], scope: str = GLIOMA_WORKFLOW_SCOPE) -> dict[str, Any]:
    """Retain separate citation partitions and explicitly identify unsupplied source results."""
    question = _question(question)
    requested = _source_names(requested_sources)
    if scope != GLIOMA_WORKFLOW_SCOPE:
        raise ArgumentError("glioma workflow only supports public research metadata scope")
    source_rows = _json_copy(sources, "glioma source inputs")
    if not isinstance(source_rows, list) or len(source_rows) > len(_SOURCES):
        raise ArgumentError("glioma sources must be a bounded list")
    partitions: dict[str, dict[str, Any]] = {}
    for source in source_rows:
        if not isinstance(source, Mapping) or set(source) != {"operation_id", "bundle"}:
            raise ArgumentError("glioma source input requires operation_id and bundle only")
        operation_id = source["operation_id"]
        if not isinstance(operation_id, str) or operation_id not in requested or operation_id in partitions:
            raise ArgumentError("glioma source is unrequested, repeated, or unsupported")
        if not isinstance(source["bundle"], Mapping):
            raise ArgumentError("glioma source bundle must be an object")
        partitions[operation_id] = _source_partition(operation_id, source["bundle"])
    missing_sources = sorted(set(requested) - partitions.keys())
    for operation_id in missing_sources:
        partitions[operation_id] = {"operation_id": operation_id, "status": "not_supplied", "record_count": None, "bundle_digest": None, "citations": []}
    available = [row for row in partitions.values() if row["status"] == "records_present"]
    empty_sources = sorted(row["operation_id"] for row in partitions.values() if row["status"] == "empty")
    unsigned = {
        "schema": GLIOMA_RECONCILIATION_SCHEMA,
        "scope": scope,
        "question": question,
        "question_digest": content_digest(question),
        "question_used_as_source_query": False,
        "status": "metadata_ready_for_review" if not missing_sources and not empty_sources and available else "needs_evidence",
        "requested_sources": sorted(requested),
        "source_partitions": [partitions[key] for key in sorted(partitions)],
        "citation_index": [citation for key in sorted(partitions) for citation in partitions[key]["citations"]],
        "missing_sources": missing_sources,
        "empty_sources": empty_sources,
        "unassessed_research_dimensions": list(_UNASSESSED),
        "source_authenticity": "not_established_by_local_reconciliation",
        "human_review_required": True,
        "clinical_actions_authorized": False,
        "clinical_interpretation": False,
        "biological_evidence_promoted": False,
        "limitations": list(_LIMITATIONS),
    }
    return {**unsigned, "reconciliation_digest": content_digest(unsigned)}


def glioma_evidence_review_handoff(reconciliation: Mapping[str, Any]) -> dict[str, Any]:
    """Bind a deterministic review handoff to the exact local reconciliation artifact."""
    value = _json_copy(reconciliation, "glioma reconciliation")
    if not isinstance(value, dict) or value.get("schema") != GLIOMA_RECONCILIATION_SCHEMA:
        raise ArgumentError("glioma review requires a reconciliation artifact")
    digest = value.pop("reconciliation_digest", None)
    if digest != content_digest(value):
        raise ArgumentError("glioma reconciliation digest does not match its contents")
    if value.get("scope") != GLIOMA_WORKFLOW_SCOPE or value.get("human_review_required") is not True or any(value.get(field) is not False for field in ("clinical_actions_authorized", "clinical_interpretation", "biological_evidence_promoted")):
        raise ArgumentError("glioma reconciliation research boundary is invalid")
    if set(value) != {"schema", "scope", "question", "question_digest", "question_used_as_source_query", "status", "requested_sources", "source_partitions", "citation_index", "missing_sources", "empty_sources", "unassessed_research_dimensions", "source_authenticity", "human_review_required", "clinical_actions_authorized", "clinical_interpretation", "biological_evidence_promoted", "limitations"}:
        raise ArgumentError("glioma reconciliation fields are invalid")
    if value["question_digest"] != content_digest(_question(value["question"])) or value["question_used_as_source_query"] is not False:
        raise ArgumentError("glioma reconciliation question binding is invalid")
    if value["limitations"] != list(_LIMITATIONS) or value["unassessed_research_dimensions"] != list(_UNASSESSED) or value["source_authenticity"] != "not_established_by_local_reconciliation":
        raise ArgumentError("glioma reconciliation non-claims are invalid")
    requested = _source_names(value.get("requested_sources"))
    partitions = value.get("source_partitions")
    if not isinstance(partitions, list) or len(partitions) != len(requested) or sorted(row.get("operation_id", "") for row in partitions if isinstance(row, Mapping)) != sorted(requested):
        raise ArgumentError("glioma reconciliation source partitions are invalid")
    citations: list[dict[str, Any]] = []
    for partition in partitions:
        operation_id = partition["operation_id"]
        partition_citations = partition.get("citations")
        if partition.get("status") not in {"records_present", "empty", "not_supplied"} or not isinstance(partition_citations, list):
            raise ArgumentError("glioma reconciliation source state is invalid")
        if partition["status"] == "not_supplied":
            if partition.get("record_count") is not None or partition.get("bundle_digest") is not None or partition_citations:
                raise ArgumentError("glioma reconciliation missing source cannot carry observations")
        else:
            if type(partition.get("record_count")) is not int or partition["record_count"] != len(partition_citations) or (partition["status"] == "empty") != (not partition_citations):
                raise ArgumentError("glioma reconciliation record count is inconsistent")
        for citation in partition_citations:
            if not isinstance(citation, dict) or citation.get("operation_id") != operation_id or citation.get("citation_url") != _citation_url(operation_id, citation.get("record_id", "")) or not re.fullmatch(r"[0-9a-f]{64}", str(citation.get("record_digest", ""))):
                raise ArgumentError("glioma reconciliation citation identity is invalid")
        citations.extend(partition_citations)
    if citations != value["citation_index"]:
        raise ArgumentError("glioma reconciliation citation index does not match source partitions")
    missing_sources = sorted(row["operation_id"] for row in partitions if row["status"] == "not_supplied")
    empty_sources = sorted(row["operation_id"] for row in partitions if row["status"] == "empty")
    expected_status = "metadata_ready_for_review" if not missing_sources and not empty_sources else "needs_evidence"
    if value["missing_sources"] != missing_sources or value["empty_sources"] != empty_sources or value["status"] != expected_status:
        raise ArgumentError("glioma reconciliation summary does not match source states")
    unsigned = {
        "schema": GLIOMA_REVIEW_HANDOFF_SCHEMA,
        "scope": GLIOMA_WORKFLOW_SCOPE,
        "status": "awaiting_human_review" if value.get("status") == "metadata_ready_for_review" else "needs_evidence",
        "question": value["question"],
        "question_digest": value["question_digest"],
        "reconciliation_digest": digest,
        "source_partitions": partitions,
        "citation_index": value["citation_index"],
        "missing_sources": value["missing_sources"],
        "empty_sources": value["empty_sources"],
        "unassessed_research_dimensions": value["unassessed_research_dimensions"],
        "review_checklist": [
            "Inspect every cited original record and source retrieval time.",
            "Review bounded-query omissions, missing metadata, source overlap, and duplicate publications.",
            "Evaluate scientific support, contradictions, and negative findings from the source material; metadata counts establish none of these.",
            "Keep registry activity separate from outcomes, and aggregate project coverage separate from measured molecular evidence.",
            "Obtain qualified review before making a scientific claim; this artifact grants no clinical or laboratory authority.",
        ],
        "human_review_required": True,
        "clinical_actions_authorized": False,
        "clinical_interpretation": False,
        "biological_evidence_promoted": False,
        "limitations": value["limitations"],
    }
    return {**unsigned, "handoff_digest": content_digest(unsigned)}


def glioma_evidence_workflow_proposal(question: str, *, include_tcga: bool = True, literature_limit: int = 10, trial_page_size: int = 25, trial_max_pages: int = 2, source_operations: Sequence[str] | None = None) -> dict[str, Any]:
    """Build a real public-retrieval DAG without contacting a source or a model."""
    question = _question(question)
    for name, value, maximum in (("literature_limit", literature_limit, 50), ("trial_page_size", trial_page_size, 100), ("trial_max_pages", trial_max_pages, 5)):
        if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
            raise ArgumentError(f"{name} must be an integer from 1 to {maximum}")
    if not isinstance(include_tcga, bool):
        raise ArgumentError("include_tcga must be boolean")
    selected = list(_SOURCES) if include_tcga else ["pubmed.search", "clinical_trials.search"]
    if source_operations is not None:
        selected = _source_names(source_operations)
        if not include_tcga and "gdc.projects" in selected:
            raise ArgumentError("explicit TCGA source conflicts with include_tcga=False")
    steps = [
        {"id": "literature", "operation": "pubmed.search", "arguments": {"specialty_lanes": ["glioma"], "per_specialty_limit": literature_limit}, "depends_on": []},
        {"id": "trials", "operation": "clinical_trials.search", "arguments": {"condition_lanes": ["glioma"], "page_size": trial_page_size, "max_pages": trial_max_pages}, "depends_on": []},
    ]
    if include_tcga:
        steps.append({"id": "tcga", "operation": "gdc.projects", "arguments": {"project_ids": ["TCGA-GBM", "TCGA-LGG"]}, "depends_on": []})
    steps = [step for step in steps if step["operation"] in selected]
    source_steps = list(steps)
    steps.append({
        "id": "reconcile",
        "operation": GLIOMA_RECONCILE_OPERATION,
        "arguments": {"question": question, "scope": GLIOMA_WORKFLOW_SCOPE, "requested_sources": [step["operation"] for step in source_steps], "sources": [{"operation_id": step["operation"], "bundle": {"$ref": f"{step['id']}#"}} for step in source_steps]},
        "depends_on": [step["id"] for step in source_steps],
    })
    steps.append({"id": "review", "operation": GLIOMA_REVIEW_OPERATION, "arguments": {"reconciliation": {"$ref": "reconcile#"}}, "depends_on": ["reconcile"]})
    return {"name": "Glioma public evidence review", "steps": steps, "outputs": ["review"]}


def register_glioma_workflow_operations(catalog: Any) -> Any:
    """Extend the general workflow catalogue with two side-effect-free local operations."""
    from .workflow_integrations import WorkflowOperation, WorkflowOperationResult

    def reconcile(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        value = reconcile_glioma_evidence(**dict(arguments))
        return WorkflowOperationResult(value=value, receipt={"operation_id": GLIOMA_RECONCILE_OPERATION, "effect": "local_read_only", "reconciliation_digest": value["reconciliation_digest"], "human_review_required": True})

    def review(arguments: Mapping[str, Any], *, approve_source_dispatch: bool = False, retrieved_at: str | None = None) -> Any:
        value = glioma_evidence_review_handoff(arguments["reconciliation"])
        return WorkflowOperationResult(value=value, receipt={"operation_id": GLIOMA_REVIEW_OPERATION, "effect": "local_read_only", "handoff_digest": value["handoff_digest"], "human_review_required": True})

    catalog.register(WorkflowOperation(
        operation_id=GLIOMA_RECONCILE_OPERATION,
        description="Reconcile separate public glioma citation, registry, and aggregate project metadata with explicit omissions; no scientific or clinical interpretation.",
        input_schema={"type": "object", "required": ["question", "requested_sources", "sources"], "additionalProperties": False, "properties": {
            "question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "scope": {"type": "string", "enum": [GLIOMA_WORKFLOW_SCOPE]},
            "requested_sources": {"type": "array", "minItems": 1, "maxItems": 3, "items": {"type": "string", "enum": list(_SOURCES)}},
            "sources": {"type": "array", "maxItems": 3, "items": {"type": "object", "required": ["operation_id", "bundle"], "additionalProperties": False, "properties": {"operation_id": {"type": "string", "enum": list(_SOURCES)}, "bundle": {"type": "object"}}}},
        }},
        output_schema={"type": "object", "required": ["schema", "scope", "status", "source_partitions", "citation_index", "reconciliation_digest"], "properties": {"schema": {"type": "string"}, "scope": {"type": "string"}, "status": {"type": "string"}, "source_partitions": {"type": "array"}, "citation_index": {"type": "array"}, "reconciliation_digest": {"type": "string"}}},
        integration_id="local.glioma_research", implementation="local",
    ), reconcile)
    catalog.register(WorkflowOperation(
        operation_id=GLIOMA_REVIEW_OPERATION,
        description="Produce a digest-bound glioma metadata review handoff with original citations, missing data, and explicit human review requirements.",
        input_schema={"type": "object", "required": ["reconciliation"], "additionalProperties": False, "properties": {"reconciliation": {"type": "object"}}},
        output_schema={"type": "object", "required": ["schema", "scope", "status", "citation_index", "handoff_digest"], "properties": {"schema": {"type": "string"}, "scope": {"type": "string"}, "status": {"type": "string"}, "citation_index": {"type": "array"}, "handoff_digest": {"type": "string"}}},
        integration_id="local.glioma_research", implementation="local",
    ), review)
    return catalog
