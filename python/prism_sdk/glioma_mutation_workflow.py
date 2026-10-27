"""Replayable review of fixed public GDC TP53 mutation-affected case aggregates.

This repository-defined composition retains project partitions and native count pointers.
It supplies neither prevalence nor individual-level or causal interpretation. Consistency
digests do not authenticate source truth. The existing catalogue-only molecular schema is
unchanged; these aggregate observations have their own schemas and explicit registration.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from . import reviewed_gdc_gene_mutation_retrieval as source

GLIOMA_MUTATION_SCOPE = "public_glioma_tp53_mutation_case_aggregate_review"
GLIOMA_MUTATION_ANALYSIS_SCHEMA = "aurora-glioma-mutation-count-analysis/0.1"
GLIOMA_MUTATION_REVIEW_SCHEMA = "aurora-glioma-mutation-count-review/0.1"
GLIOMA_MUTATION_ANALYSIS_OPERATION = "glioma.mutation.analyze"
GLIOMA_MUTATION_REVIEW_OPERATION = "glioma.mutation.review_handoff"
MAX_GLIOMA_MUTATION_ARTIFACT_BYTES = 2_000_000
_LIMITATIONS = [
    "The question labels this fixed reviewed query; it does not change its gene or project selection.",
    "Mutation-affected case aggregates are source observations, not mutation events, assay values, prevalence, outcomes or causal evidence.",
    "A missing project/gene bucket is unknown; an explicit observed zero remains an observed count.",
    "No gene-specific assay denominator is established. Rates, comparisons of rates, and cross-project pooling are not calculated.",
    "TCGA-GBM and TCGA-LGG remain separate source partitions; cohort independence, overlap with other sources and scientific validity are unassessed.",
    "The fixed Ensembl gene ID and source label do not supply an Entrez crosswalk or validate identifier equivalence.",
    "Self-computed consistency digests do not authenticate the provider or scientific truth; human scientific review remains required.",
    "No individual records, patient intake, diagnosis, treatment selection, enrollment or laboratory actions are supplied.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(value)
        if len(text.encode("utf-8")) > MAX_GLIOMA_MUTATION_ARTIFACT_BYTES:
            raise ArgumentError("glioma mutation artifact exceeds its byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (TypeError, ValueError, RecursionError, UnicodeError) as error:
        if isinstance(error, ArgumentError):
            raise
        raise ArgumentError("glioma mutation artifact must be bounded canonical JSON") from error


def _question(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError("glioma mutation question must be nonempty text")
    try:
        if len(value.encode("utf-8")) > 8000:
            raise ArgumentError("glioma mutation question exceeds its byte bound")
    except UnicodeError as error:
        raise ArgumentError("glioma mutation question must be valid Unicode") from error
    return value


def analyze_glioma_mutation_counts(*, question: str, bundle: Mapping[str, Any]) -> dict[str, Any]:
    """Replay the native source projection and retain counts without imputing missingness."""
    question = _question(question)
    native = source.validate_reviewed_gdc_gene_mutation_bundle(_safe(bundle))
    provenance = native["sources"][0]
    citations, projects = [], []
    for row in native["projects"]:
        observed = row["state"] == "observed"
        citation_id = None
        if observed:
            citation = {"source_id": row["source_id"],
                "project_id": row["project_id"], "gene_id": row["gene_id"],
                "uri": provenance["uri"], "retrieved_at": provenance["retrieved_at"],
                "source_json_pointer": row["source_json_pointer"],
                "case_count_with_mutation": row["case_count_with_mutation"],
                "native_observation_digest": content_digest(row), "source_bundle_digest": native["bundle_digest"]}
            citation_id = "gdc_tp53:" + content_digest(citation)
            citations.append({"citation_id": citation_id, **citation})
        projects.append({"project_id": row["project_id"], "lane": row["lane"], "state": row["state"],
            "case_count_with_mutation": row["case_count_with_mutation"], "citation_id": citation_id,
            "denominator": None, "denominator_state": "not_established", "prevalence": None,
            "native_observation": row})
    unsigned = {"schema": GLIOMA_MUTATION_ANALYSIS_SCHEMA, "scope": GLIOMA_MUTATION_SCOPE,
        "question": question, "status": "aggregate_counts_ready_for_review" if native["observed_count"] == 2 else "incomplete_aggregate_counts",
        "gene_id": native["gene_id"], "gene_symbol": native["gene_symbol"],
        "projects": projects, "observed_project_count": native["observed_count"],
        "unobserved_project_count": 2 - native["observed_count"], "citation_index": citations,
        "source_bundle_digest": native["bundle_digest"], "source_set_digest": native["source_set_digest"],
        "native_bundle": native, "aggregate_count_values_retrieved": native["observed_count"] > 0,
        "individual_records_retrieved": False, "prevalence_calculated": False, "counts_pooled": False,
        "cohort_overlap": "unassessed", "entrez_crosswalk": "not_supplied",
        "scientific_support_established": False, "human_review_required": True,
        "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "analysis_digest": content_digest(unsigned)})


def validate_glioma_mutation_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    """Replay all derived fields, rather than accepting a restamped outer digest."""
    actual = _safe(value)
    if not isinstance(actual, dict) or actual.get("schema") != GLIOMA_MUTATION_ANALYSIS_SCHEMA:
        raise ArgumentError("glioma mutation analysis schema is invalid")
    if "question" not in actual or "native_bundle" not in actual:
        raise ArgumentError("glioma mutation analysis lacks its replay inputs")
    expected = analyze_glioma_mutation_counts(question=actual["question"], bundle=actual["native_bundle"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma mutation analysis does not replay from the native bundle")
    return expected


def glioma_mutation_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    """Return a scientific review hold, including when an explicit count is zero."""
    checked = validate_glioma_mutation_analysis(analysis)
    unsigned = {"schema": GLIOMA_MUTATION_REVIEW_SCHEMA, "scope": GLIOMA_MUTATION_SCOPE,
        "status": "awaiting_scientific_human_review" if checked["observed_project_count"] > 0 else "needs_evidence",
        "question": checked["question"], "analysis": checked, "analysis_digest": checked["analysis_digest"],
        "citation_index": checked["citation_index"], "human_review_required": True,
        "scientific_support_established": False, "clinical_actions_authorized": False,
        "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "handoff_digest": content_digest(unsigned)})


def validate_glioma_mutation_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    """Rebuild the complete handoff, including its citations and review hold."""
    actual = _safe(value)
    if not isinstance(actual, dict) or "analysis" not in actual:
        raise ArgumentError("glioma mutation handoff lacks its replay analysis")
    expected = glioma_mutation_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma mutation handoff does not replay from analysis")
    return expected


def register_glioma_mutation_workflow_operations(catalog: Any) -> Any:
    """Register local operations only; source onboarding remains explicit and separate."""
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise ArgumentError("glioma mutation registration requires a workflow catalogue")
    operations = catalog.operations()
    if any(operation.operation_id in {GLIOMA_MUTATION_ANALYSIS_OPERATION, GLIOMA_MUTATION_REVIEW_OPERATION}
           for operation in operations):
        raise ArgumentError("glioma mutation local operations are already registered")
    integrations = {operation.integration_id for operation in operations}
    if (len(operations) + 2 > MAX_WORKFLOW_OPERATIONS
            or "local.glioma_mutation" not in integrations and len(integrations) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("glioma mutation registration would exceed catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = analyze_glioma_mutation_counts(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_MUTATION_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only", "human_review_required": True})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_mutation_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_MUTATION_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only", "human_review_required": True})
    catalog.register(WorkflowOperation(GLIOMA_MUTATION_ANALYSIS_OPERATION,
        "Replay fixed public GDC TP53 mutation-affected case aggregates for GBM/LGG; preserve missing counts, exact source pointers and review holds without rates or pooling.",
        {"type": "object", "required": ["question", "bundle"], "additionalProperties": False,
         "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000}, "bundle": {"type": "object"}}},
        {"type": "object", "required": ["schema", "scope", "analysis_digest", "projects", "citation_index", "native_bundle"],
         "properties": {"schema": {"const": GLIOMA_MUTATION_ANALYSIS_SCHEMA}, "scope": {"const": GLIOMA_MUTATION_SCOPE},
            "analysis_digest": {"type": "string"}, "projects": {"type": "array"}, "citation_index": {"type": "array"}, "native_bundle": {"type": "object"}}},
        "local.glioma_mutation", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_MUTATION_REVIEW_OPERATION,
        "Replay aggregate count analysis into a citation-preserving scientific human-review hold; no scientific or clinical approval.",
        {"type": "object", "required": ["analysis"], "additionalProperties": False, "properties": {"analysis": {"type": "object"}}},
        {"type": "object", "required": ["schema", "scope", "status", "handoff_digest", "analysis"],
         "properties": {"schema": {"const": GLIOMA_MUTATION_REVIEW_SCHEMA}, "scope": {"const": GLIOMA_MUTATION_SCOPE},
            "status": {"enum": ["needs_evidence", "awaiting_scientific_human_review"]}, "handoff_digest": {"type": "string"}, "analysis": {"type": "object"}}},
        "local.glioma_mutation", implementation="local"), review)
    return catalog


def glioma_mutation_workflow_proposal(question: str, *, timeout_ms: int = 30_000) -> dict[str, Any]:
    """Compose one reviewed aggregate read and two local stages, with no dispatch."""
    question = _question(question)
    source.ReviewedGdcGeneMutationRetrievalConfig(timeout_ms=timeout_ms)
    return {"name": "Glioma TP53 aggregate mutation count review", "steps": [
        {"id": "counts", "operation": "gdc.gene_mutation_counts", "arguments": {"timeout_ms": timeout_ms}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_MUTATION_ANALYSIS_OPERATION,
         "arguments": {"question": question, "bundle": {"$ref": "counts#"}}, "depends_on": ["counts"]},
        {"id": "review", "operation": GLIOMA_MUTATION_REVIEW_OPERATION,
         "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_mutation_workflow(question: str, catalog: Any, *, timeout_ms: int = 30_000,
                                     requirements: Any = None) -> Any:
    """Bind the exact native bundles and read limit to the retained research question."""
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_mutation_workflow_proposal(question, timeout_ms=timeout_ms)
    operations = [step["operation"] for step in proposal["steps"]]
    native = compile_workflow_requirements(question, required_operations=operations,
        output_operations=[GLIOMA_MUTATION_REVIEW_OPERATION], max_steps=3,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": "gdc.gene_mutation_counts", "path": "/timeout_ms", "comparison": "equals", "value": timeout_ms},
                        {"operation": GLIOMA_MUTATION_ANALYSIS_OPERATION, "path": "/question", "comparison": "equals", "value": question}],
        data_links=[{"source_operation": "gdc.gene_mutation_counts", "target_operation": GLIOMA_MUTATION_ANALYSIS_OPERATION,
                     "source_pointer": "", "target_path": "/bundle"},
                    {"source_operation": GLIOMA_MUTATION_ANALYSIS_OPERATION, "target_operation": GLIOMA_MUTATION_REVIEW_OPERATION,
                     "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None:
        native = merge_workflow_requirements(question, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=question, requirements=native)
