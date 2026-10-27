"""Replay TP53 aggregates against GDC SSM-data coverage without manufacturing rates.

Repository-defined application extension: no available detailed blueprint module specifies
these public aggregate contracts. This is a denominator admissibility assessment, not an
assay eligibility model, clinical analysis, population join or prevalence estimator.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_gdc_gene_mutation_retrieval import validate_reviewed_gdc_gene_mutation_bundle
from .reviewed_gdc_ssm_coverage_retrieval import (
    GDC_SSM_COVERAGE_OPERATION, GDC_SSM_COVERAGE_PROJECTS, validate_gdc_ssm_coverage_bundle,
)

GLIOMA_DENOMINATOR_SCOPE = "public_glioma_TP53_denominator_admissibility"
GLIOMA_DENOMINATOR_ANALYSIS_SCHEMA = "aurora-glioma-denominator-assessment/0.1"
GLIOMA_DENOMINATOR_REVIEW_SCHEMA = "aurora-glioma-denominator-review/0.1"
GLIOMA_DENOMINATOR_ANALYSIS_OPERATION = "glioma.denominator.assess"
GLIOMA_DENOMINATOR_REVIEW_OPERATION = "glioma.denominator.review_handoff"
_MUTATION = "gdc.gene_mutation_counts"
_LIMITATIONS = [
    "The question labels these fixed reviewed TP53 and project-coverage queries; it does not supply arbitrary source filters or extract general scientific requirements.",
    "A source case_with_ssm availability count is not a gene-specific assayed/callable population, including when it is numerically greater than the TP53-affected case count.",
    "Exact project label agreement compares source assertions. No individual population join, overlap assessment, independent scientific equivalence or same-release guarantee is established.",
    "The arithmetic count-order check is a necessary numerical diagnostic only; a passing inequality cannot establish denominator admissibility.",
    "Source release, reference build, assay design, callable regions, eligibility exclusions and non-observation ascertainment are not supplied by the reviewed aggregate contracts.",
    "Missing acquisition, missing requested buckets/fields and explicitly observed zero remain separate states; none permits an imputed denominator or negative biological conclusion.",
    "Native inputs, pointers, retrieval clocks and digests remain inspectable. Native consistency does not authenticate source truth or establish scientific sufficiency.",
    "TCGA-GBM/LGG remain separate. No prevalence, ratio, population subtraction, pooled count, clinical action or laboratory action is calculated or authorized.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(text.encode("utf-8")) > 8_000_000:
            raise ArgumentError("glioma denominator artifact exceeds its byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        raise ArgumentError("glioma denominator artifact must be bounded canonical JSON") from None


def _question(value: Any) -> str:
    try:
        valid = type(value) is str and bool(value.strip()) and "\x00" not in value and len(value.encode("utf-8")) <= 8000
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("glioma denominator question must be bounded nonempty text")
    return value


def _citation(bundle: dict[str, Any], operation: str, *, pointer: str,
        source_pointer: str | None, value: Any, kind: str) -> dict[str, Any]:
    mutation = operation == _MUTATION
    source = bundle["sources"][0] if mutation else bundle["source_inputs"][0]
    unsigned = {"operation_id": operation, "record_kind": kind, "artifact_pointer": pointer,
        "source_json_pointer": source_pointer,
        "pointer_authority": ("validated_source_projection" if mutation else
            "validated_missing_project_projection" if source_pointer is None else "retained_response_native_payload"),
        "source_uri": source["uri"] if mutation else source["url"],
        "retrieved_at": bundle["generated_at"] if mutation else bundle["retrieved_at"],
        "source_bundle_digest": bundle["bundle_digest"], "source_value": value,
        "source_value_digest": content_digest(value)}
    return {"citation_id": "glioma_denominator:" + content_digest(unsigned), **unsigned}


def assess_glioma_denominator(*, question: str, mutation_bundle: Mapping[str, Any] | None = None,
        coverage_bundle: Mapping[str, Any] | None = None) -> dict[str, Any]:
    """Assess native count availability and necessary ordering while withholding a rate."""
    question = _question(question)
    mutation = validate_reviewed_gdc_gene_mutation_bundle(_safe(mutation_bundle)) if mutation_bundle is not None else None
    coverage = validate_gdc_ssm_coverage_bundle(_safe(coverage_bundle)) if coverage_bundle is not None else None
    projects, citations = [], []
    for index, project_id in enumerate(GDC_SSM_COVERAGE_PROJECTS):
        numerator = None if mutation is None else mutation["projects"][index]
        available = None if coverage is None else coverage["projects"][index]
        numerator_state = "source_not_supplied" if numerator is None else numerator["state"]
        availability_state = "source_not_supplied" if available is None else available["state"]
        count = None if numerator is None else numerator["case_count_with_mutation"]
        candidate = None if available is None else available["cases_with_ssm_data"]
        citation_ids = []
        if numerator is not None:
            citation = _citation(mutation, _MUTATION, pointer=f"/projects/{index}",
                source_pointer=numerator["source_json_pointer"], value=numerator, kind="TP53_mutation_count_observation")
            citations.append(citation)
            citation_ids.append(citation["citation_id"])
        if available is not None:
            native_pointer = available["native_project_pointer"]
            if native_pointer is None:
                pointer, value = "/projects/" + str(index), available
            else:
                bucket_index = int(native_pointer.rsplit("/", 1)[1])
                pointer = "/source_inputs/0/native_payload" + native_pointer
                value = coverage["source_inputs"][0]["native_payload"]["aggregations"]["projects"]["buckets"][bucket_index]
            citation = _citation(coverage, GDC_SSM_COVERAGE_OPERATION, pointer=pointer,
                source_pointer=native_pointer, value=value, kind="project_SSM_data_availability")
            citations.append(citation)
            citation_ids.append(citation["citation_id"])
        both_counts = count is not None and candidate is not None
        numerical = ("not_evaluable_missing_count" if not both_counts else
            "candidate_count_less_than_numerator" if candidate < count else "necessary_count_order_holds")
        blockers = ["gene_specific_assay_eligibility_not_supplied", "same_release_not_established",
            "reference_build_not_established", "assay_callability_and_exclusions_not_supplied",
            "cohort_membership_and_overlap_unassessed", "non_observation_ascertainment_not_supplied"]
        if numerator_state != "observed":
            blockers.insert(0, "numerator_not_observed")
        if availability_state != "observed":
            blockers.insert(0, "candidate_availability_not_observed")
        if numerical == "candidate_count_less_than_numerator":
            blockers.insert(0, "necessary_numerical_count_order_failed")
        projects.append({"project_id": project_id, "lane": "gbm" if index == 0 else "lgg",
            "numerator": {"state": numerator_state, "case_count_with_mutation": count,
                "native_observation": numerator},
            "candidate_availability": {"state": availability_state, "cases_with_ssm_data": candidate,
                "semantic_kind": "project_cases_with_associated_ssm_data", "native_observation": available},
            "assessment": {"state": "denominator_not_established", "blockers": blockers,
                "necessary_count_order_check": numerical,
                "zero_candidate_observed": availability_state == "observed" and candidate == 0,
                "gene_specific_denominator": None, "prevalence": None, "prevalence_calculated": False,
                "population_join_performed": False, "scientific_comparability": "unestablished"},
            "source_project_labels": {"mutation": None if numerator is None else numerator["project_id"],
                "coverage": None if available is None else available["project_id"],
                "exact_value_agreement": None if numerator is None or available is None else numerator["project_id"] == available["project_id"],
                "scientific_population_equivalence": "unestablished"}, "citation_ids": citation_ids})
    missing_sources = [operation for operation, value in ((_MUTATION, mutation), (GDC_SSM_COVERAGE_OPERATION, coverage)) if value is None]
    unsigned = {"schema": GLIOMA_DENOMINATOR_ANALYSIS_SCHEMA, "scope": GLIOMA_DENOMINATOR_SCOPE,
        "question": question, "question_used_as_source_query": False,
        "status": "needs_evidence" if missing_sources else "denominator_not_established",
        "gene_id": "ENSG00000141510", "gene_symbol": "TP53", "projects": projects,
        "source_inputs": {"mutation_bundle": mutation, "coverage_bundle": coverage},
        "missing_sources": missing_sources, "citation_index": citations,
        "acquisition_context": {"mutation_retrieved_at": None if mutation is None else mutation["generated_at"],
            "coverage_retrieved_at": None if coverage is None else coverage["retrieved_at"],
            "same_release": "not_established", "release_identity_supplied": False,
            "coverage_query_gene_id": None if coverage is None else coverage["query"]["gene_id"],
            "coverage_gene_filter_supplied": False},
        "denominator": None, "prevalence_calculated": False, "counts_pooled": False,
        "cohort_overlap": "unassessed", "individual_records_retrieved": False,
        "scientific_support_established": False, "human_review_required": True,
        "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "analysis_digest": content_digest(unsigned)})


def validate_glioma_denominator_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        if set(actual["source_inputs"]) != {"mutation_bundle", "coverage_bundle"}:
            raise ArgumentError("glioma denominator source fields changed")
        expected = assess_glioma_denominator(question=actual["question"], **actual["source_inputs"])
    except (KeyError, TypeError):
        raise ArgumentError("glioma denominator analysis lacks native replay inputs") from None
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma denominator analysis does not replay from native inputs")
    return expected


def glioma_denominator_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    checked = validate_glioma_denominator_analysis(analysis)
    unsigned = {"schema": GLIOMA_DENOMINATOR_REVIEW_SCHEMA, "scope": GLIOMA_DENOMINATOR_SCOPE,
        "status": "needs_evidence" if checked["missing_sources"] else "awaiting_scientific_human_review",
        "analysis": checked, "analysis_digest": checked["analysis_digest"], "citation_index": checked["citation_index"],
        "human_review_required": True, "scientific_support_established": False,
        "clinical_actions_authorized": False,
        "review_actions": ["Inspect each native numerator and availability count, its state, exact source pointer and acquisition clock.",
            "Do not substitute associated-SSM data or total project population for a TP53 callable/eligible assay denominator.",
            "Obtain gene-specific assay eligibility, comparable cohort/release/build, exclusions and non-observation semantics before a qualified scientific review of any rate."],
        "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "handoff_digest": content_digest(unsigned)})


def validate_glioma_denominator_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    if not isinstance(actual, dict) or "analysis" not in actual:
        raise ArgumentError("glioma denominator handoff lacks replay analysis")
    expected = glioma_denominator_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("glioma denominator handoff does not replay from analysis")
    return expected


def register_glioma_denominator_workflow_operations(catalog: Any) -> Any:
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise ArgumentError("glioma denominator registration requires a workflow catalogue")
    operations = catalog.operations()
    if {row.operation_id for row in operations} & {GLIOMA_DENOMINATOR_ANALYSIS_OPERATION, GLIOMA_DENOMINATOR_REVIEW_OPERATION}:
        raise ArgumentError("glioma denominator operations already registered")
    if (len(operations)+2 > MAX_WORKFLOW_OPERATIONS or "local.glioma_denominator" not in {row.integration_id for row in operations}
            and len({row.integration_id for row in operations}) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("glioma denominator registration exceeds catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = assess_glioma_denominator(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_DENOMINATOR_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only"})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_denominator_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_DENOMINATOR_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only"})
    catalog.register(WorkflowOperation(GLIOMA_DENOMINATOR_ANALYSIS_OPERATION,
        "Replay separate public GDC TP53 mutation-affected case counts and project associated-SSM-data coverage into a denominator admissibility assessment; native count-order diagnostic, no prevalence or population join.",
        {"type": "object", "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "mutation_bundle": {"type": ["object", "null"]}, "coverage_bundle": {"type": ["object", "null"]}},
         "required": ["question", "mutation_bundle", "coverage_bundle"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "analysis_digest", "source_inputs", "citation_index"],
         "properties": {"schema": {"const": GLIOMA_DENOMINATOR_ANALYSIS_SCHEMA}, "scope": {"const": GLIOMA_DENOMINATOR_SCOPE},
            "analysis_digest": {"type": "string"}, "source_inputs": {"type": "object"}, "citation_index": {"type": "array"}}},
        "local.glioma_denominator", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_DENOMINATOR_REVIEW_OPERATION,
        "Replay a TP53 denominator admissibility assessment into a scientific human-review hold; unavailable gene-specific assay denominators never become rates.",
        {"type": "object", "required": ["analysis"], "properties": {"analysis": {"type": "object"}}, "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "handoff_digest", "analysis", "status"],
         "properties": {"schema": {"const": GLIOMA_DENOMINATOR_REVIEW_SCHEMA}, "scope": {"const": GLIOMA_DENOMINATOR_SCOPE},
            "handoff_digest": {"type": "string"}, "analysis": {"type": "object"}, "status": {"enum": ["needs_evidence", "awaiting_scientific_human_review"]}}},
        "local.glioma_denominator", implementation="local"), review)
    return catalog


def glioma_denominator_workflow_proposal(question: str) -> dict[str, Any]:
    question = _question(question)
    return {"name": "Glioma TP53 denominator admissibility", "steps": [
        {"id": "mutations", "operation": _MUTATION, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "coverage", "operation": GDC_SSM_COVERAGE_OPERATION, "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_DENOMINATOR_ANALYSIS_OPERATION,
            "arguments": {"question": question, "mutation_bundle": {"$ref": "mutations#"}, "coverage_bundle": {"$ref": "coverage#"}},
            "depends_on": ["mutations", "coverage"]},
        {"id": "review", "operation": GLIOMA_DENOMINATOR_REVIEW_OPERATION,
            "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_denominator_workflow(task: str, catalog: Any, requirements: Any = None) -> Any:
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_denominator_workflow_proposal(task)
    operations = [row["operation"] for row in proposal["steps"]]
    native = compile_workflow_requirements(task, required_operations=operations,
        output_operations=[GLIOMA_DENOMINATOR_REVIEW_OPERATION], max_steps=4,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": proposal["steps"][index]["operation"], "path": "", "comparison": "equals",
            "value": proposal["steps"][index]["arguments"]} for index in (0, 1)]
            + [{"operation": GLIOMA_DENOMINATOR_ANALYSIS_OPERATION, "path": "/question", "comparison": "equals", "value": task}],
        data_links=[{"source_operation": source, "target_operation": GLIOMA_DENOMINATOR_ANALYSIS_OPERATION,
            "source_pointer": "", "target_path": target} for source, target in ((_MUTATION, "/mutation_bundle"),
                (GDC_SSM_COVERAGE_OPERATION, "/coverage_bundle"))]
            + [{"source_operation": GLIOMA_DENOMINATOR_ANALYSIS_OPERATION, "target_operation": GLIOMA_DENOMINATOR_REVIEW_OPERATION,
                "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None:
        native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=task, requirements=native)
