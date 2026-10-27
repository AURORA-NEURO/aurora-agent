"""Local TP53 source-reference comparison and scientific review hold.

This repository-defined composition has no available blueprint identifier. It replays reviewed
Ensembl and NCBI annotation bundles, compares exact source-provided Entrez strings, and retains
ambiguity and missingness. Source agreement is not scientific identifier-equivalence proof and
never joins measurements, cohorts or clinical decisions. Existing molecular schemas are unchanged.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .glioma_molecular_workflow import _replay as _replay_molecular_source
from .reviewed_ensembl_gene_retrieval import validate_ensembl_gene_bundle
from .reviewed_ncbi_gene_retrieval import REVIEWED_NCBI_GENE_CATALOGUE


GLIOMA_GENE_REFERENCE_SCOPE = "public_glioma_TP53_gene_reference_review"
GLIOMA_GENE_REFERENCE_ANALYSIS_SCHEMA = "aurora-glioma-gene-reference-analysis/0.1"
GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA = "aurora-glioma-gene-reference-review/0.1"
GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION = "glioma.gene_references.analyze"
GLIOMA_GENE_REFERENCE_REVIEW_OPERATION = "glioma.gene_references.review_handoff"
MAX_GLIOMA_GENE_REFERENCE_BYTES = 2_000_000
_ENSEMBL = "ensembl.gene_references"
_NCBI = "ncbi_gene.summary"
_LIMITATIONS = [
    "The retained question labels this fixed TP53 source comparison; it does not refine source selectors or interpret arbitrary requests.",
    "Exact source-provided Entrez string agreement is a measured annotation agreement, not independently validated scientific identifier equivalence.",
    "All Ensembl Entrez candidates remain candidates; aliases, case folding and numeric coercion never supply a match.",
    "A missing bundle is not an observed empty reference array, a failed request, or negative biological evidence.",
    "Neither source reports exhaustive cross-database identity coverage; distinct source observations are not evidence of scientific independence.",
    "The existing molecular crosswalk remains unchanged; no measurements, associations, cohorts, mutation counts or outcomes are joined.",
    "Consistency digests and local normalization replay do not authenticate the provider or establish scientific truth.",
    "Scientific human review is required; individual records, clinical actions, laboratory actions and preclinical execution authority are absent.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(value)
        if len(text.encode("utf-8")) > MAX_GLIOMA_GENE_REFERENCE_BYTES:
            raise ArgumentError("glioma gene-reference artifact exceeds its byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (TypeError, ValueError, UnicodeError, RecursionError, OverflowError) as error:
        if isinstance(error, ArgumentError):
            raise
        raise ArgumentError("glioma gene-reference artifact must be bounded canonical JSON") from None


def _question(value: Any) -> str:
    try:
        valid = isinstance(value, str) and bool(value.strip()) and "\x00" not in value and len(value.encode("utf-8")) <= 8000
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("glioma gene-reference question must be bounded nonempty text")
    return value


def _ncbi_bundle(value: Mapping[str, Any]) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    native = _safe(value)
    if not isinstance(native, dict):
        raise ArgumentError("NCBI TP53 source must be a native bundle object")
    records, coverage = _replay_molecular_source(_NCBI, native)
    if len(records) != 1:
        raise ArgumentError("gene-reference comparison requires exactly the reviewed NCBI TP53 row")
    row = records[0]["data"]
    if (row.get("symbol") != "TP53" or type(row.get("symbol")) is not str
            or row.get("gene_id") != REVIEWED_NCBI_GENE_CATALOGUE["TP53"]
            or type(row.get("gene_id")) is not str
            or type(row.get("organism_taxid")) is not int or row["organism_taxid"] != 9606):
        raise ArgumentError("NCBI gene identity is outside the exact reviewed human TP53 contract")
    return native, records[0], coverage


def _citation(*, operation: str, record_kind: str, native_pointer: str, source_uri: str,
              timestamp: str, bundle_digest: str, source_value: Any) -> dict[str, Any]:
    payload = {"operation_id": operation, "record_kind": record_kind,
               "native_pointer": native_pointer, "source_uri": source_uri, "retrieved_at": timestamp,
               "source_bundle_digest": bundle_digest, "source_value": source_value,
               "source_value_digest": content_digest(source_value)}
    return {"citation_id": "gene_ref:" + content_digest(payload), **payload}


def analyze_glioma_gene_references(*, question: str, ensembl_bundle: Mapping[str, Any] | None = None,
                                   ncbi_bundle: Mapping[str, Any] | None = None) -> dict[str, Any]:
    """Compare exact source assertions after native replay; None alone means not supplied."""
    question = _question(question)
    native_ensembl = validate_ensembl_gene_bundle(_safe(ensembl_bundle)) if ensembl_bundle is not None else None
    native_ncbi, ncbi_record, ncbi_coverage = _ncbi_bundle(ncbi_bundle) if ncbi_bundle is not None else (None, None, None)
    citations, candidates, partitions = [], [], []
    gene_citation_id = ncbi_citation_id = None
    if native_ensembl is None:
        partitions.append({"operation_id": _ENSEMBL, "state": "not_supplied", "bundle_digest": None,
                           "returned_gene_count": None, "returned_reference_count": None, "coverage": None,
                           "omissions": None})
    else:
        gene_citation = _citation(operation=_ENSEMBL, record_kind="source_gene_identity",
            native_pointer="/source_inputs/0/native_payload", source_uri=native_ensembl["receipts"][0]["source_uri"],
            timestamp=native_ensembl["retrieved_at"], bundle_digest=native_ensembl["bundle_digest"],
            source_value=native_ensembl["source_inputs"][0]["native_payload"])
        citations.append(gene_citation)
        gene_citation_id = gene_citation["citation_id"]
        partitions.append({"operation_id": _ENSEMBL, "state": "observed", "bundle_digest": native_ensembl["bundle_digest"],
            "returned_gene_count": 1, "returned_reference_count": len(native_ensembl["entrez_candidates"]),
            "coverage": native_ensembl["coverage"],
            "omissions": {"reported_total_count": None, "unreturned_reference_count": None,
                          "reference_exhaustiveness": "unassessed", "source_limitations": native_ensembl["limitations"]}})
        for index, row in enumerate(native_ensembl["entrez_candidates"]):
            citation = _citation(operation=_ENSEMBL, record_kind="source_EntrezGene_reference_candidate",
                native_pointer=f"/source_inputs/1/native_payload/{index}", source_uri=native_ensembl["receipts"][1]["source_uri"],
                timestamp=native_ensembl["retrieved_at"], bundle_digest=native_ensembl["bundle_digest"],
                source_value=native_ensembl["source_inputs"][1]["native_payload"][index])
            citations.append(citation)
            expected = ncbi_record["record_id"] if ncbi_record is not None else None
            candidates.append({"candidate_index": index, "ensembl_gene_id": row["ensembl_gene_id"],
                "entrez_gene_id": row["entrez_gene_id"], "source_candidate": row,
                "citation_id": citation["citation_id"], "ncbi_expected_entrez_id": expected,
                "comparison_state": "not_compared_missing_ncbi" if expected is None else
                                    "exact_id_value_agreement" if row["entrez_gene_id"] == expected else "id_value_mismatch",
                "scientific_identifier_equivalence": "unestablished"})
        if not candidates:
            citations.append(_citation(operation=_ENSEMBL, record_kind="observed_empty_reference_array",
                native_pointer="/source_inputs/1/native_payload", source_uri=native_ensembl["receipts"][1]["source_uri"],
                timestamp=native_ensembl["retrieved_at"], bundle_digest=native_ensembl["bundle_digest"], source_value=[]))
    if native_ncbi is None:
        partitions.append({"operation_id": _NCBI, "state": "not_supplied", "bundle_digest": None,
                           "returned_gene_count": None, "returned_reference_count": None, "coverage": None,
                           "omissions": None})
    else:
        ncbi_citation = _citation(operation=_NCBI, record_kind="source_gene_identity", native_pointer="/genes/0",
            source_uri=ncbi_record["uri"], timestamp=native_ncbi["generated_at"],
            bundle_digest=native_ncbi["bundle_digest"], source_value=ncbi_record["data"])
        citations.append(ncbi_citation)
        ncbi_citation_id = ncbi_citation["citation_id"]
        partitions.append({"operation_id": _NCBI, "state": "observed", "bundle_digest": native_ncbi["bundle_digest"],
            "returned_gene_count": 1, "returned_reference_count": None, "coverage": ncbi_coverage,
            "omissions": {"unrequested_catalogue_symbols": ncbi_coverage["unrequested_catalogue_symbols"],
                          "source_limitations": native_ncbi["limitations"], "cross_database_reference_coverage": "not_supplied"}})
    both = native_ensembl is not None and native_ncbi is not None
    matching = sum(row["comparison_state"] == "exact_id_value_agreement" for row in candidates) if both else None
    mismatching = sum(row["comparison_state"] == "id_value_mismatch" for row in candidates) if both else None
    if not both:
        comparison_state = "not_comparable_missing_source"
    elif not candidates:
        comparison_state = "observed_no_reference_candidates"
    elif len(candidates) > 1:
        comparison_state = "multiple_candidates_with_matching_id" if matching else "multiple_candidates_without_matching_id"
    else:
        comparison_state = "single_candidate_id_value_agreement" if matching else "single_candidate_id_value_mismatch"
    unsigned = {"schema": GLIOMA_GENE_REFERENCE_ANALYSIS_SCHEMA, "scope": GLIOMA_GENE_REFERENCE_SCOPE,
        "question": question, "question_used_as_source_query": False,
        "status": "source_assertions_ready_for_review" if both else "needs_evidence",
        "source_inputs": {"ensembl_bundle": native_ensembl, "ncbi_bundle": native_ncbi},
        "source_partitions": partitions, "missing_sources": [row["operation_id"] for row in partitions if row["state"] == "not_supplied"],
        "gene_symbol": "TP53", "ensembl_gene_id": native_ensembl["gene"]["ensembl_gene_id"] if native_ensembl is not None else None,
        "ncbi_entrez_gene_id": ncbi_record["record_id"] if ncbi_record is not None else None,
        "gene_citation_id": gene_citation_id, "ncbi_citation_id": ncbi_citation_id,
        "candidate_comparisons": candidates,
        "comparison": {"state": comparison_state, "source_pair_observed": both,
            "candidate_count": len(candidates) if native_ensembl is not None else None,
            "matching_candidate_count": matching, "mismatching_candidate_count": mismatching,
            "multiple_candidates": len(candidates) > 1 if native_ensembl is not None else None,
            "scientific_identifier_equivalence": "unestablished", "independent_validation": "not_supplied",
            "case_folding_performed": False, "alias_matching_performed": False, "numeric_coercion_performed": False},
        "citation_index": citations, "citation_count": len(citations), "omitted_citation_count": 0,
        "measurements_joined": False, "cohorts_joined": False, "scientific_support_established": False,
        "human_review_required": True, "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "analysis_digest": content_digest(unsigned)})


def validate_glioma_gene_reference_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    """Replay every derived field from retained source bundles, with no source dispatch."""
    actual = _safe(value)
    if not isinstance(actual, dict) or actual.get("schema") != GLIOMA_GENE_REFERENCE_ANALYSIS_SCHEMA:
        raise ArgumentError("glioma gene-reference analysis schema is invalid")
    if not isinstance(actual.get("source_inputs"), dict) or set(actual["source_inputs"]) != {"ensembl_bundle", "ncbi_bundle"} or "question" not in actual:
        raise ArgumentError("glioma gene-reference analysis lacks its exact replay inputs")
    expected = analyze_glioma_gene_references(question=actual["question"], **actual["source_inputs"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma gene-reference analysis does not replay from native sources")
    return expected


def glioma_gene_reference_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    """Retain source agreement, discrepancies and omissions under a scientific review hold."""
    checked = validate_glioma_gene_reference_analysis(analysis)
    unsigned = {"schema": GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA, "scope": GLIOMA_GENE_REFERENCE_SCOPE,
        "status": "needs_evidence" if checked["missing_sources"] else "awaiting_scientific_human_review",
        "question": checked["question"], "analysis": checked, "analysis_digest": checked["analysis_digest"],
        "comparison_state": checked["comparison"]["state"], "citation_index": checked["citation_index"],
        "human_review_required": True, "scientific_support_established": False,
        "scientific_identifier_equivalence": "unestablished", "clinical_actions_authorized": False,
        "review_actions": ["Inspect every native source value, source timestamp, omission and exact citation pointer.",
                           "Resolve multiple candidates, disagreements and missing evidence without alias or numeric coercion.",
                           "Obtain independent scientific identity validation before joining any measurement or cohort plane."],
        "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "handoff_digest": content_digest(unsigned)})


def validate_glioma_gene_reference_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    if not isinstance(actual, dict) or "analysis" not in actual:
        raise ArgumentError("glioma gene-reference handoff lacks its replay analysis")
    expected = glioma_gene_reference_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma gene-reference handoff does not replay from analysis")
    return expected


def register_glioma_gene_reference_workflow_operations(catalog: Any) -> Any:
    """Register two local operations only; reviewed source registration remains explicit."""
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise ArgumentError("gene-reference registration requires a workflow catalogue")
    if any(operation.operation_id in {GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION, GLIOMA_GENE_REFERENCE_REVIEW_OPERATION}
           for operation in catalog.operations()):
        raise ArgumentError("gene-reference local operations are already registered")
    operations = catalog.operations()
    integration_ids = {operation.integration_id for operation in operations}
    if (len(operations) + 2 > MAX_WORKFLOW_OPERATIONS
            or "local.glioma_gene_references" not in integration_ids and len(integration_ids) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("gene-reference local registration would exceed catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> WorkflowOperationResult:
        value = analyze_glioma_gene_references(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only", "human_review_required": True})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> WorkflowOperationResult:
        value = glioma_gene_reference_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_GENE_REFERENCE_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only", "human_review_required": True})
    catalog.register(WorkflowOperation(GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION,
        "Replay reviewed public TP53 Ensembl and NCBI annotations; compare every exact source Entrez candidate without scientific equivalence, alias coercion or measurement joins.",
        {"type": "object", "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "ensembl_bundle": {"type": ["object", "null"]}, "ncbi_bundle": {"type": ["object", "null"]}},
         "required": ["question", "ensembl_bundle", "ncbi_bundle"], "additionalProperties": False},
        {"type": "object", "properties": {"schema": {"const": GLIOMA_GENE_REFERENCE_ANALYSIS_SCHEMA},
            "scope": {"const": GLIOMA_GENE_REFERENCE_SCOPE}, "analysis_digest": {"type": "string"},
            "source_inputs": {"type": "object"}, "comparison": {"type": "object"}, "candidate_comparisons": {"type": "array"},
            "citation_index": {"type": "array"}, "human_review_required": {"const": True}},
         "required": ["schema", "scope", "analysis_digest", "source_inputs", "comparison", "candidate_comparisons", "citation_index", "human_review_required"]},
        "local.glioma_gene_references", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_GENE_REFERENCE_REVIEW_OPERATION,
        "Replay the exact TP53 source-reference comparison into a citation-preserving scientific human-review hold; source agreement does not validate biological identity.",
        {"type": "object", "properties": {"analysis": {"type": "object"}}, "required": ["analysis"], "additionalProperties": False},
        {"type": "object", "properties": {"schema": {"const": GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA},
            "scope": {"const": GLIOMA_GENE_REFERENCE_SCOPE}, "handoff_digest": {"type": "string"},
            "analysis": {"type": "object"}, "status": {"enum": ["needs_evidence", "awaiting_scientific_human_review"]},
            "human_review_required": {"const": True}},
         "required": ["schema", "scope", "handoff_digest", "analysis", "status", "human_review_required"]},
        "local.glioma_gene_references", implementation="local"), review)
    return catalog


def glioma_gene_reference_workflow_proposal(question: str) -> dict[str, Any]:
    question = _question(question)
    return {"name": "Glioma TP53 source gene-reference review", "steps": [
        {"id": "ensembl", "operation": _ENSEMBL, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "ncbi", "operation": _NCBI, "arguments": {"gene_symbols": ["TP53"], "timeout_ms": 30_000}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION,
         "arguments": {"question": question, "ensembl_bundle": {"$ref": "ensembl#"}, "ncbi_bundle": {"$ref": "ncbi#"}},
         "depends_on": ["ensembl", "ncbi"]},
        {"id": "review", "operation": GLIOMA_GENE_REFERENCE_REVIEW_OPERATION,
         "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_gene_reference_workflow(task: str, catalog: Any, requirements: Any = None) -> Any:
    """Bind exact source arguments, whole native bundles, review output and call budgets."""
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_gene_reference_workflow_proposal(task)
    operations = [step["operation"] for step in proposal["steps"]]
    native = compile_workflow_requirements(task, required_operations=operations,
        output_operations=[GLIOMA_GENE_REFERENCE_REVIEW_OPERATION], max_steps=4,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": proposal["steps"][index]["operation"], "path": "", "comparison": "equals",
                         "value": proposal["steps"][index]["arguments"]} for index in (0, 1)]
            + [{"operation": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION, "path": "/question", "comparison": "equals", "value": task}],
        data_links=[{"source_operation": _ENSEMBL, "target_operation": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION,
                     "source_pointer": "", "target_path": "/ensembl_bundle"},
                    {"source_operation": _NCBI, "target_operation": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION,
                     "source_pointer": "", "target_path": "/ncbi_bundle"},
                    {"source_operation": GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION, "target_operation": GLIOMA_GENE_REFERENCE_REVIEW_OPERATION,
                     "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None:
        native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=task, requirements=native)
