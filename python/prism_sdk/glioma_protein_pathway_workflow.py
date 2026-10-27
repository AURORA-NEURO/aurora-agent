"""Replay public TP53 protein/pathway context under a scientific review hold.

Repository-defined application extension; no detailed blueprint identifier is available.
General human source annotations are not glioma measurements, disease activity, enrichment,
scientific identifier equivalence or clinical support. The question labels a fixed query.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_reactome_retrieval import validate_reactome_pathway_bundle
from .reviewed_uniprot_retrieval import validate_uniprot_annotation_bundle

GLIOMA_PROTEIN_PATHWAY_SCOPE = "public_glioma_TP53_protein_pathway_context"
GLIOMA_PROTEIN_PATHWAY_ANALYSIS_SCHEMA = "aurora-glioma-protein-pathway-analysis/0.1"
GLIOMA_PROTEIN_PATHWAY_REVIEW_SCHEMA = "aurora-glioma-protein-pathway-review/0.1"
GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION = "glioma.protein_pathways.analyze"
GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION = "glioma.protein_pathways.review_handoff"
_UNIPROT, _REACTOME = "uniprot.protein_annotation", "reactome.pathway_annotations"
MAX_GLIOMA_PROTEIN_PATHWAY_BYTES = 8_000_000
_LIMITATIONS = [
    "The question labels fixed human P04637 annotation context and does not refine source selectors or interpret arbitrary requests.",
    "Exact accession value agreement compares source assertions; scientific identifier equivalence and independent validation remain unestablished.",
    "All returned protein and gene names remain source candidates; no preferred alias, case folding or name-based identity join is performed.",
    "General human pathway annotations and source disease/inferred flags do not measure glioma pathway activity, alteration, enrichment or causality.",
    "Missing sources, missing fields and observed empty arrays are separate states; none establishes negative biological evidence.",
    "Source coverage remains unassessed. Lower-level pathway mapping does not establish an exhaustive hierarchy, relevance ranking or annotation quality.",
    "Citations are retained in source order up to the explicit cap; omitted citation identities/digest remain inspectable and their influence is unknown.",
    "Native source digests and replay check internal consistency, not authenticated truth, scientific sufficiency or clinical support.",
    "No measurements, cohorts, mutation counts, individual records, sequences, mechanisms, clinical or laboratory actions are joined or authorized.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(value)
        if len(text.encode("utf-8")) > MAX_GLIOMA_PROTEIN_PATHWAY_BYTES:
            raise ArgumentError("protein/pathway artifact exceeds its byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError) as error:
        if isinstance(error, ArgumentError):
            raise
        raise ArgumentError("protein/pathway artifact must be bounded canonical JSON") from None


def _question(value: Any) -> str:
    try:
        valid = type(value) is str and bool(value.strip()) and "\x00" not in value and len(value.encode("utf-8")) <= 8000
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("protein/pathway question must be bounded nonempty text")
    return value


def _pointer(value: Any, path: str) -> Any:
    for part in path.split("/")[1:]:
        part = part.replace("~1", "/").replace("~0", "~")
        value = value[int(part)] if isinstance(value, list) else value[part]
    return value


def _citation(bundle: dict[str, Any], operation: str, path: str, kind: str) -> dict[str, Any]:
    source = bundle["source_inputs"][0]
    value = _pointer(source["native_payload"], path)
    payload = {"operation_id": operation, "record_kind": kind,
        "native_pointer": "/source_inputs/0/native_payload" + path,
        "source_uri": source["url"], "retrieved_at": bundle["retrieved_at"],
        "source_bundle_digest": bundle["bundle_digest"], "source_value": value,
        "source_value_digest": content_digest(value)}
    return {"citation_id": "protein_pathway:" + content_digest(payload), **payload}


def analyze_glioma_protein_pathways(*, question: str, uniprot_bundle: Mapping[str, Any] | None = None,
        reactome_bundle: Mapping[str, Any] | None = None, max_citations: int = 64) -> dict[str, Any]:
    """Replay each native source and compare exact assertions without biological inference."""
    question = _question(question)
    if type(max_citations) is not int or not 1 <= max_citations <= 1024:
        raise ArgumentError("protein/pathway citation cap must be an integer from one to 1024")
    protein = validate_uniprot_annotation_bundle(_safe(uniprot_bundle)) if uniprot_bundle is not None else None
    pathway = validate_reactome_pathway_bundle(_safe(reactome_bundle)) if reactome_bundle is not None else None
    citations, partitions, names, pathways = [], [], [], []
    for operation, bundle in ((_UNIPROT, protein), (_REACTOME, pathway)):
        partitions.append({"operation_id": operation, "state": "not_supplied" if bundle is None else "observed",
            "bundle_digest": None if bundle is None else bundle["bundle_digest"],
            "coverage": None if bundle is None else bundle["coverage"],
            "limitations": None if bundle is None else bundle["limitations"]})
    if protein is not None:
        citations.append(_citation(protein, _UNIPROT, "/primaryAccession", "source_primary_accession"))
        for collection in ("protein_names", "gene_name_candidates"):
            for row in protein[collection]:
                citation = _citation(protein, _UNIPROT, row["source_pointer"], collection)
                citations.append(citation)
                names.append({"kind": collection, "source_candidate": row, "citation_id": citation["citation_id"],
                    "identity_match_performed": False})
    if pathway is not None:
        if not pathway["pathways"]:
            citations.append(_citation(pathway, _REACTOME, "", "observed_empty_pathway_array"))
        for row in pathway["pathways"]:
            citation = _citation(pathway, _REACTOME, row["source_pointer"], "source_pathway_annotation")
            citations.append(citation)
            pathways.append({"stable_id": row["stable_id"], "stable_version": row["stable_version"],
                "display_name": row["display_name"], "source_is_in_disease": row["source_is_in_disease"],
                "source_is_inferred": row["source_is_inferred"], "citation_id": citation["citation_id"],
                "glioma_activity": "unmeasured", "glioma_specificity": "not_established"})
    both = protein is not None and pathway is not None
    comparison = {"state": "not_comparable_missing_source" if not both else
        "exact_accession_value_agreement" if protein["entry"]["primary_accession"] == pathway["query"]["identifier"] else "accession_value_mismatch",
        "uniprot_primary_accession": None if protein is None else protein["entry"]["primary_accession"],
        "reactome_query_accession": None if pathway is None else pathway["query"]["identifier"],
        "scientific_identifier_equivalence": "unestablished", "independent_validation": "not_supplied"}
    omitted = citations[max_citations:]
    unsigned = {"schema": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_SCHEMA, "scope": GLIOMA_PROTEIN_PATHWAY_SCOPE,
        "question": question, "question_used_as_source_query": False,
        "status": "source_assertions_ready_for_review" if both else "needs_evidence",
        "source_inputs": {"uniprot_bundle": protein, "reactome_bundle": pathway}, "source_partitions": partitions,
        "missing_sources": [row["operation_id"] for row in partitions if row["state"] == "not_supplied"],
        "source_name_candidates": names, "pathway_annotations": pathways, "comparison": comparison,
        "field_observations": None if protein is None else protein["field_observations"],
        "citation_index": citations[:max_citations], "citation_count": min(len(citations), max_citations),
        "citation_policy": {"max_citations": max_citations, "ordering": "uniprot_native_then_reactome_native",
            "selection_is_relevance_ranking": False, "candidate_citation_count": len(citations),
            "omitted_citation_count": len(omitted), "omitted_citation_ids": [row["citation_id"] for row in omitted],
            "omitted_citations_digest": content_digest(omitted), "omitted_influence": "unknown" if omitted else "none_omitted"},
        "measurements_joined": False, "cohorts_joined": False, "scientific_support_established": False,
        "human_review_required": True, "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "analysis_digest": content_digest(unsigned)})


def validate_glioma_protein_pathway_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        if set(actual["source_inputs"]) != {"uniprot_bundle", "reactome_bundle"}:
            raise ArgumentError("protein/pathway replay input fields changed")
        expected = analyze_glioma_protein_pathways(question=actual["question"], **actual["source_inputs"],
            max_citations=actual["citation_policy"]["max_citations"])
    except (KeyError, TypeError):
        raise ArgumentError("protein/pathway analysis lacks native replay inputs") from None
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("protein/pathway analysis does not replay from native sources")
    return expected


def glioma_protein_pathway_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    checked = validate_glioma_protein_pathway_analysis(analysis)
    unsigned = {"schema": GLIOMA_PROTEIN_PATHWAY_REVIEW_SCHEMA, "scope": GLIOMA_PROTEIN_PATHWAY_SCOPE,
        "status": "needs_evidence" if checked["missing_sources"] else "awaiting_scientific_human_review",
        "analysis": checked, "analysis_digest": checked["analysis_digest"], "citation_index": checked["citation_index"],
        "human_review_required": True, "scientific_support_established": False, "clinical_actions_authorized": False,
        "review_actions": ["Inspect native annotation values, timestamps, source flags and exact citation pointers.",
            "Resolve missing fields and unknown omissions; no source annotation establishes glioma activity or negative evidence.",
            "Obtain independent scientific identity validation and appropriate measurements before any scientific conclusion."],
        "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "handoff_digest": content_digest(unsigned)})


def validate_glioma_protein_pathway_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    if not isinstance(actual, dict) or "analysis" not in actual:
        raise ArgumentError("protein/pathway handoff lacks replay analysis")
    expected = glioma_protein_pathway_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("protein/pathway handoff does not replay from analysis")
    return expected


def register_glioma_protein_pathway_workflow_operations(catalog: Any) -> Any:
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise ArgumentError("protein/pathway registration requires a workflow catalogue")
    operations = catalog.operations()
    ids = {row.operation_id for row in operations}
    if ids & {GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION, GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION}:
        raise ArgumentError("protein/pathway operations already registered")
    if (len(operations)+2 > MAX_WORKFLOW_OPERATIONS or "local.glioma_protein_pathways" not in {row.integration_id for row in operations}
            and len({row.integration_id for row in operations}) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("protein/pathway registration exceeds catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = analyze_glioma_protein_pathways(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only"})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_protein_pathway_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only"})
    catalog.register(WorkflowOperation(GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION,
        "Replay fixed public TP53 UniProt and Reactome annotations into protein/pathway context with exact source citations and unknown omissions; no glioma activity or enrichment inference.",
        {"type": "object", "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "uniprot_bundle": {"type": ["object", "null"]}, "reactome_bundle": {"type": ["object", "null"]},
            "max_citations": {"type": "integer", "minimum": 1, "maximum": 1024}},
         "required": ["question", "uniprot_bundle", "reactome_bundle"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "analysis_digest", "source_inputs", "citation_index"],
         "properties": {"schema": {"const": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_SCHEMA}, "scope": {"const": GLIOMA_PROTEIN_PATHWAY_SCOPE},
            "analysis_digest": {"type": "string"}, "source_inputs": {"type": "object"}, "citation_index": {"type": "array"}}},
        "local.glioma_protein_pathways", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION,
        "Replay TP53 protein/pathway context into a scientific human-review hold; annotations do not establish disease activity or scientific support.",
        {"type": "object", "required": ["analysis"], "properties": {"analysis": {"type": "object"}}, "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "handoff_digest", "analysis", "status"],
         "properties": {"schema": {"const": GLIOMA_PROTEIN_PATHWAY_REVIEW_SCHEMA}, "scope": {"const": GLIOMA_PROTEIN_PATHWAY_SCOPE},
            "handoff_digest": {"type": "string"}, "analysis": {"type": "object"}, "status": {"enum": ["needs_evidence", "awaiting_scientific_human_review"]}}},
        "local.glioma_protein_pathways", implementation="local"), review)
    return catalog


def glioma_protein_pathway_workflow_proposal(question: str) -> dict[str, Any]:
    question = _question(question)
    return {"name": "Glioma TP53 protein/pathway annotation context", "steps": [
        {"id": "uniprot", "operation": _UNIPROT, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "reactome", "operation": _REACTOME, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION,
         "arguments": {"question": question, "uniprot_bundle": {"$ref": "uniprot#"}, "reactome_bundle": {"$ref": "reactome#"}, "max_citations": 64}, "depends_on": ["uniprot", "reactome"]},
        {"id": "review", "operation": GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION,
         "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_protein_pathway_workflow(task: str, catalog: Any, requirements: Any = None) -> Any:
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_protein_pathway_workflow_proposal(task)
    operations = [row["operation"] for row in proposal["steps"]]
    native = compile_workflow_requirements(task, required_operations=operations,
        output_operations=[GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION], max_steps=4,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": proposal["steps"][index]["operation"], "path": "", "comparison": "equals", "value": proposal["steps"][index]["arguments"]} for index in (0, 1)]
            + [{"operation": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION, "path": "/question", "comparison": "equals", "value": task},
               {"operation": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION, "path": "/max_citations", "comparison": "equals", "value": 64}],
        data_links=[{"source_operation": source, "target_operation": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION,
            "source_pointer": "", "target_path": target} for source, target in ((_UNIPROT, "/uniprot_bundle"), (_REACTOME, "/reactome_bundle"))]
            + [{"source_operation": GLIOMA_PROTEIN_PATHWAY_ANALYSIS_OPERATION, "target_operation": GLIOMA_PROTEIN_PATHWAY_REVIEW_OPERATION, "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None:
        native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=task, requirements=native)
