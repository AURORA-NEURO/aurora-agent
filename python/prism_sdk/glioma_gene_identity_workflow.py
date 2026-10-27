"""Three-source TP53 metadata concordance with explicit identity uncertainty.

Repository-defined extension: no available blueprint module specifies this source composition.
This extends the existing native Ensembl/NCBI comparison with HGNC nomenclature/status evidence;
source string concordance never authorizes a scientific identity, cohort or measurement join.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .glioma_gene_reference_workflow import analyze_glioma_gene_references
from .reviewed_hgnc_gene_retrieval import HGNC_GENE_OPERATION_ID, validate_hgnc_gene_reference_bundle

GLIOMA_GENE_IDENTITY_SCOPE = "public_glioma_TP53_three_source_metadata_concordance"
GLIOMA_GENE_IDENTITY_ANALYSIS_SCHEMA = "aurora-glioma-gene-identity-analysis/0.1"
GLIOMA_GENE_IDENTITY_REVIEW_SCHEMA = "aurora-glioma-gene-identity-review/0.1"
GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION = "glioma.gene_identity.analyze"
GLIOMA_GENE_IDENTITY_REVIEW_OPERATION = "glioma.gene_identity.review_handoff"
_ENSEMBL, _NCBI = "ensembl.gene_references", "ncbi_gene.summary"
_LIMITATIONS = [
    "The question labels fixed TP53 queries; it does not extract arbitrary scientific requirements or select another gene, resource or assay.",
    "Exact case-sensitive identifier strings are compared as source assertions only. No alias matching, case folding, numeric conversion or preferred candidate selection is performed.",
    "HGNC approved nomenclature/status is source metadata, not independent biological equivalence, disease relevance, glioma molecular classification or scientific validation.",
    "Missing acquisition/fields, explicit null/empty values, no records, withdrawn records, ambiguous records and source-window omissions remain different states.",
    "All source records, aliases, previous names and Ensembl reference candidates remain native and inspectable; none is silently collapsed into a selected identity.",
    "Ensembl/NCBI source limitations and omissions remain intact. Source annotations may share upstream curation; cross-source independence is unassessed.",
    "Citations have a bounded explicit prefix policy; omitted citation identities/digest are retained with unknown influence while all native observations remain.",
    "Source clocks differ and source release equivalence is unestablished. Self-computed digests establish internal consistency, not authenticated truth or scientific sufficiency.",
    "No sequences, raw variants, individual records, assay values, cohorts, clinical or laboratory actions are joined or authorized.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(text.encode("utf-8")) > 8_000_000:
            raise ArgumentError("gene identity artifact exceeds its byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        raise ArgumentError("gene identity artifact must be bounded canonical JSON") from None


def _compare(observation: dict[str, Any], other: str | None) -> str:
    if other is None: return "not_comparable_missing_source"
    if observation["state"] != "observed": return "not_comparable_unobserved_field"
    return "exact_string_agreement" if observation["value"] == other else "string_mismatch"


def _citation(bundle: dict[str, Any], path: str, value: Any, kind: str) -> dict[str, Any]:
    source = bundle["source_inputs"][0]
    payload = {"operation_id": HGNC_GENE_OPERATION_ID, "record_kind": kind,
        "native_pointer": "/source_inputs/0/native_payload"+path, "source_uri": source["url"],
        "retrieved_at": bundle["retrieved_at"], "source_bundle_digest": bundle["bundle_digest"],
        "source_value": value, "source_value_digest": content_digest(value)}
    return {"citation_id": "gene_identity:"+content_digest(payload), **payload}


def analyze_glioma_gene_identity(*, question: str, hgnc_bundle: Mapping[str, Any] | None = None,
        ensembl_bundle: Mapping[str, Any] | None = None, ncbi_bundle: Mapping[str, Any] | None = None,
        max_citations: int = 128) -> dict[str, Any]:
    """Replay each source and every exact identifier candidate without resolving identity."""
    if type(max_citations) is not int or not 1 <= max_citations <= 1024:
        raise ArgumentError("gene identity citation cap must be one to 1024")
    base = analyze_glioma_gene_references(question=question, ensembl_bundle=ensembl_bundle, ncbi_bundle=ncbi_bundle)
    hgnc = validate_hgnc_gene_reference_bundle(_safe(hgnc_bundle)) if hgnc_bundle is not None else None
    citations = list(base["citation_index"])
    partitions = list(base["source_partitions"])
    partitions.append({"operation_id": HGNC_GENE_OPERATION_ID, "state": "not_supplied" if hgnc is None else "observed",
        "bundle_digest": None if hgnc is None else hgnc["bundle_digest"],
        "identity_state": None if hgnc is None else hgnc["coverage"]["selection_state"],
        "coverage": None if hgnc is None else hgnc["coverage"],
        "omissions": None if hgnc is None else {"unreturned_reported_count": hgnc["coverage"]["unreturned_reported_count"],
            "influence": hgnc["coverage"]["unreturned_influence"], "source_limitations": hgnc["limitations"]}})
    candidate_rows = []
    if hgnc is not None:
        if not hgnc["records"]:
            citations.append(_citation(hgnc, "/response/docs", [], "observed_empty_source_document_array"))
        for row in hgnc["records"]:
            root = _citation(hgnc, row["native_pointer"], row["native_record"], "source_nomenclature_record")
            citations.append(root)
            field_citations = {}
            for field, observation in row["field_observations"].items():
                if observation["native_pointer"] is not None:
                    citation = _citation(hgnc, observation["native_pointer"], observation["value"], "source_"+field)
                    citations.append(citation)
                    field_citations[field] = citation["citation_id"]
            observations = row["field_observations"]
            reference_checks = [{"candidate_index": candidate["candidate_index"],
                "ensembl_entrez_candidate": candidate["entrez_gene_id"],
                "ensembl_candidate_citation_id": candidate["citation_id"],
                "state": _compare(observations["entrez_id"], candidate["entrez_gene_id"]),
                "scientific_identifier_equivalence": "unestablished"} for candidate in base["candidate_comparisons"]]
            candidate_rows.append({"record_index": row["record_index"], "source_record_state": row["state"],
                "source_selection_state": hgnc["coverage"]["selection_state"],
                "source_identity_ready": hgnc["coverage"]["source_identity_ready"],
                "field_observations": observations, "native_record": row["native_record"],
                "record_citation_id": root["citation_id"], "field_citation_ids": field_citations,
                "ensembl_gene_id_comparison": {"state": _compare(observations["ensembl_gene_id"], base["ensembl_gene_id"]),
                    "ensembl_gene_id": base["ensembl_gene_id"], "hgnc_asserted_value": observations["ensembl_gene_id"]["value"]},
                "ncbi_entrez_id_comparison": {"state": _compare(observations["entrez_id"], base["ncbi_entrez_gene_id"]),
                    "ncbi_entrez_gene_id": base["ncbi_entrez_gene_id"], "hgnc_asserted_value": observations["entrez_id"]["value"]},
                "all_ensembl_entrez_candidate_comparisons": reference_checks,
                "aliases_used_to_resolve_identity": False, "scientific_identifier_equivalence": "unestablished"})
    missing = [row["operation_id"] for row in partitions if row["state"] == "not_supplied"]
    if missing:
        status = "needs_evidence"
    elif hgnc["coverage"]["source_identity_ready"] is not True:
        status = "needs_identity_resolution"
    elif any(row[key]["state"] == "not_comparable_unobserved_field" for row in candidate_rows
            for key in ("ensembl_gene_id_comparison", "ncbi_entrez_id_comparison")):
        status = "needs_evidence"
    elif any(row[key]["state"] == "string_mismatch" for row in candidate_rows
            for key in ("ensembl_gene_id_comparison", "ncbi_entrez_id_comparison")) or any(
                item["state"] == "string_mismatch" for row in candidate_rows for item in row["all_ensembl_entrez_candidate_comparisons"]):
        status = "metadata_discrepancy_for_review"
    else:
        status = "source_assertions_ready_for_review"
    omitted = citations[max_citations:]
    value = {"schema": GLIOMA_GENE_IDENTITY_ANALYSIS_SCHEMA, "scope": GLIOMA_GENE_IDENTITY_SCOPE,
        "question": base["question"], "question_used_as_source_query": False, "gene_symbol": "TP53", "status": status,
        "source_inputs": {**base["source_inputs"], "hgnc_bundle": hgnc}, "source_partitions": partitions,
        "missing_sources": missing, "hgnc_candidate_observations": candidate_rows,
        "ensembl_ncbi_context": {"analysis_digest": base["analysis_digest"], "comparison": base["comparison"],
            "candidate_comparisons": base["candidate_comparisons"], "limitations": base["limitations"]},
        "comparison_policy": {"kind": "case_sensitive_source_metadata_concordance", "preferred_identity_selected": False,
            "alias_matching_performed": False, "numeric_coercion_performed": False, "case_folding_performed": False,
            "scientific_identifier_equivalence": "unestablished", "source_independence": "unassessed", "same_release": "unestablished"},
        "citation_index": citations[:max_citations], "citation_count": min(len(citations), max_citations),
        "citation_policy": {"max_citations": max_citations, "ordering": "ensembl_ncbi_native_then_hgnc_record_field_order",
            "selection_is_relevance_ranking": False, "candidate_citation_count": len(citations),
            "omitted_citation_count": len(omitted), "omitted_citation_ids": [row["citation_id"] for row in omitted],
            "omitted_citations_digest": content_digest(omitted), "omitted_influence": "unknown" if omitted else "none_omitted"},
        "measurements_joined": False, "cohorts_joined": False, "scientific_support_established": False,
        "human_review_required": True, "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "analysis_digest": content_digest(value)})


def validate_glioma_gene_identity_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        if set(actual["source_inputs"]) != {"hgnc_bundle", "ensembl_bundle", "ncbi_bundle"}:
            raise ArgumentError("gene identity replay source fields changed")
        expected = analyze_glioma_gene_identity(question=actual["question"], **actual["source_inputs"],
            max_citations=actual["citation_policy"]["max_citations"])
    except (KeyError, TypeError):
        raise ArgumentError("gene identity analysis lacks native replay inputs") from None
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("gene identity analysis does not replay from native sources")
    return expected


def glioma_gene_identity_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    checked = validate_glioma_gene_identity_analysis(analysis)
    status = checked["status"] if checked["status"] in {"needs_evidence", "needs_identity_resolution"} else "awaiting_scientific_human_review"
    value = {"schema": GLIOMA_GENE_IDENTITY_REVIEW_SCHEMA, "scope": GLIOMA_GENE_IDENTITY_SCOPE, "status": status,
        "analysis": checked, "analysis_digest": checked["analysis_digest"], "citation_index": checked["citation_index"],
        "scientific_identifier_equivalence": "unestablished", "scientific_support_established": False,
        "human_review_required": True, "clinical_actions_authorized": False,
        "review_actions": ["Inspect every source partition, raw HGNC byte/payload record, source status, alias and exact candidate reference.",
            "Resolve missing/withdrawn/ambiguous records, discordant identifiers and unknown omissions without selecting aliases or coercing identifiers.",
            "Obtain qualified independent scientific identity validation before joining any assay, cohort or measurement."],
        "limitations": list(_LIMITATIONS)}
    return _safe({**value, "handoff_digest": content_digest(value)})


def validate_glioma_gene_identity_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    if type(actual) is not dict or "analysis" not in actual:
        raise ArgumentError("gene identity handoff lacks native replay analysis")
    expected = glioma_gene_identity_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("gene identity handoff does not replay")
    return expected


def register_glioma_gene_identity_workflow_operations(catalog: Any) -> Any:
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog): raise ArgumentError("gene identity requires a workflow catalogue")
    operations = catalog.operations()
    if {row.operation_id for row in operations} & {GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION, GLIOMA_GENE_IDENTITY_REVIEW_OPERATION}:
        raise ArgumentError("gene identity operations already registered")
    if (len(operations)+2 > MAX_WORKFLOW_OPERATIONS or "local.glioma_gene_identity" not in {row.integration_id for row in operations}
            and len({row.integration_id for row in operations}) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("gene identity registration exceeds catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = analyze_glioma_gene_identity(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only"})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_gene_identity_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_GENE_IDENTITY_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only"})
    catalog.register(WorkflowOperation(GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION,
        "Replay HGNC status/nomenclature and every Ensembl/NCBI TP53 reference candidate as source metadata concordance; retain ambiguity/withdrawal/missingness, no scientific identity join.",
        {"type": "object", "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "hgnc_bundle": {"type": ["object", "null"]}, "ensembl_bundle": {"type": ["object", "null"]},
            "ncbi_bundle": {"type": ["object", "null"]}, "max_citations": {"type": "integer", "minimum": 1, "maximum": 1024}},
            "required": ["question", "hgnc_bundle", "ensembl_bundle", "ncbi_bundle"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "analysis_digest", "source_inputs", "citation_index"],
            "properties": {"schema": {"const": GLIOMA_GENE_IDENTITY_ANALYSIS_SCHEMA}, "scope": {"const": GLIOMA_GENE_IDENTITY_SCOPE},
                "analysis_digest": {"type": "string"}, "source_inputs": {"type": "object"}, "citation_index": {"type": "array"}}},
        "local.glioma_gene_identity", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_GENE_IDENTITY_REVIEW_OPERATION,
        "Replay three-source TP53 metadata concordance into a human scientific review hold; absent/withdrawn/ambiguous identities require evidence, not a scientific join.",
        {"type": "object", "properties": {"analysis": {"type": "object"}}, "required": ["analysis"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "handoff_digest", "analysis", "status"],
            "properties": {"schema": {"const": GLIOMA_GENE_IDENTITY_REVIEW_SCHEMA}, "scope": {"const": GLIOMA_GENE_IDENTITY_SCOPE},
                "handoff_digest": {"type": "string"}, "analysis": {"type": "object"},
                "status": {"enum": ["needs_evidence", "needs_identity_resolution", "awaiting_scientific_human_review"]}}},
        "local.glioma_gene_identity", implementation="local"), review)
    return catalog


def glioma_gene_identity_workflow_proposal(question: str) -> dict[str, Any]:
    # Reuse the established bounded-question validator without acquiring source data.
    question = analyze_glioma_gene_references(question=question)["question"]
    return {"name": "Glioma TP53 three-source gene identity metadata review", "steps": [
        {"id": "hgnc", "operation": HGNC_GENE_OPERATION_ID, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "ensembl", "operation": _ENSEMBL, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "ncbi", "operation": _NCBI, "arguments": {"gene_symbols": ["TP53"], "timeout_ms": 30_000}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION,
            "arguments": {"question": question, "hgnc_bundle": {"$ref": "hgnc#"}, "ensembl_bundle": {"$ref": "ensembl#"},
                "ncbi_bundle": {"$ref": "ncbi#"}, "max_citations": 128}, "depends_on": ["hgnc", "ensembl", "ncbi"]},
        {"id": "review", "operation": GLIOMA_GENE_IDENTITY_REVIEW_OPERATION,
            "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_gene_identity_workflow(task: str, catalog: Any, requirements: Any = None) -> Any:
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_gene_identity_workflow_proposal(task)
    operations = [row["operation"] for row in proposal["steps"]]
    native = compile_workflow_requirements(task, required_operations=operations,
        output_operations=[GLIOMA_GENE_IDENTITY_REVIEW_OPERATION], max_steps=5,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": proposal["steps"][i]["operation"], "path": "", "comparison": "equals",
            "value": proposal["steps"][i]["arguments"]} for i in (0, 1, 2)]
            + [{"operation": GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION, "path": path, "comparison": "equals", "value": value}
                for path, value in (("/question", task), ("/max_citations", 128))],
        data_links=[{"source_operation": source, "target_operation": GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION,
            "source_pointer": "", "target_path": target} for source, target in ((HGNC_GENE_OPERATION_ID, "/hgnc_bundle"),
                (_ENSEMBL, "/ensembl_bundle"), (_NCBI, "/ncbi_bundle"))]
            + [{"source_operation": GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION, "target_operation": GLIOMA_GENE_IDENTITY_REVIEW_OPERATION,
                "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None: native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=task, requirements=native)
