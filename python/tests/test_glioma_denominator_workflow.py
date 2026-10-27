"""Native denominator assessment cannot promote availability or missingness into a rate."""
from copy import deepcopy

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_denominator_workflow import (
    GLIOMA_DENOMINATOR_ANALYSIS_OPERATION, GLIOMA_DENOMINATOR_REVIEW_OPERATION,
    assess_glioma_denominator, compile_glioma_denominator_workflow, glioma_denominator_review_handoff,
    register_glioma_denominator_workflow_operations, validate_glioma_denominator_analysis,
    validate_glioma_denominator_handoff,
)
from prism_sdk.reviewed_gdc_ssm_coverage_retrieval import register_gdc_ssm_coverage_operations
from prism_sdk.reviewed_gdc_gene_mutation_retrieval import register_gdc_gene_mutation_operations
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk import reviewed_gdc_gene_mutation_retrieval as mutation_source
from test_reviewed_gdc_gene_mutation_retrieval import _execute, _response, _bucket
from test_reviewed_gdc_ssm_coverage_retrieval import CLOCK, Fixture, read, payload

TASK = "Assess whether public TP53 mutation counts have an admissible denominator in TCGA-GBM/LGG."


def inputs():
    return {"mutation_bundle": _execute().bundle, "coverage_bundle": read()}


def test_observed_available_counts_and_passing_inequality_never_establish_assay_denominator():
    value = assess_glioma_denominator(question=TASK, **inputs())
    gbm = value["projects"][0]
    assert gbm["numerator"]["case_count_with_mutation"] == 10
    assert gbm["candidate_availability"]["cases_with_ssm_data"] == 15
    assert gbm["assessment"]["necessary_count_order_check"] == "necessary_count_order_holds"
    assert gbm["assessment"]["gene_specific_denominator"] is None
    assert gbm["assessment"]["prevalence"] is None
    assert "gene_specific_assay_eligibility_not_supplied" in gbm["assessment"]["blockers"]
    assert value["acquisition_context"]["same_release"] == "not_established"
    assert value["counts_pooled"] is False and value["scientific_support_established"] is False
    assert validate_glioma_denominator_analysis(value) == value


def test_explicit_zero_candidate_and_numerator_are_retained_without_division_by_zero_or_zero_rate():
    value = assess_glioma_denominator(question=TASK, **inputs())
    lgg = value["projects"][1]
    assert lgg["numerator"]["state"] == "observed" and lgg["numerator"]["case_count_with_mutation"] == 0
    assert lgg["candidate_availability"]["state"] == "observed" and lgg["candidate_availability"]["cases_with_ssm_data"] == 0
    assert lgg["assessment"]["zero_candidate_observed"] is True
    assert lgg["assessment"]["necessary_count_order_check"] == "necessary_count_order_holds"
    assert lgg["assessment"]["prevalence"] is None
    assert glioma_denominator_review_handoff(value)["status"] == "awaiting_scientific_human_review"


def test_candidate_count_below_numerator_is_an_explicit_necessary_order_failure_not_a_rate():
    native = payload()
    native["aggregations"]["projects"]["buckets"][0]["case_summary"]["case_with_ssm"]["doc_count"] = 3
    source = inputs()
    source["coverage_bundle"] = read(Fixture(native))
    value = assess_glioma_denominator(question=TASK, **source)
    assessment = value["projects"][0]["assessment"]
    assert assessment["necessary_count_order_check"] == "candidate_count_less_than_numerator"
    assert "necessary_numerical_count_order_failed" in assessment["blockers"]
    assert assessment["gene_specific_denominator"] is None and assessment["prevalence_calculated"] is False


@pytest.mark.parametrize("missing", ["mutation_bundle", "coverage_bundle", "both"])
def test_missing_acquisition_is_not_observed_empty_or_negative_evidence(missing):
    source = inputs()
    for key in source:
        if key == missing or missing == "both": source[key] = None
    value = assess_glioma_denominator(question=TASK, **source)
    assert value["status"] == "needs_evidence"
    assert value["missing_sources"]
    assert value["projects"][0]["assessment"]["necessary_count_order_check"] == "not_evaluable_missing_count"
    assert glioma_denominator_review_handoff(value)["status"] == "needs_evidence"
    assert validate_glioma_denominator_analysis(value) == value


@pytest.mark.parametrize("missing", ["project", "summary", "ssm", "count"])
def test_missing_coverage_fields_keep_count_check_unevaluable(missing):
    native = payload()
    row = native["aggregations"]["projects"]["buckets"][0]
    if missing == "project": native["aggregations"]["projects"]["buckets"].pop(0)
    elif missing == "summary": row.pop("case_summary")
    elif missing == "ssm": row["case_summary"].pop("case_with_ssm")
    else: row["case_summary"]["case_with_ssm"].pop("doc_count")
    source = inputs()
    source["coverage_bundle"] = read(Fixture(native))
    value = assess_glioma_denominator(question=TASK, **source)
    row = value["projects"][0]
    assert row["candidate_availability"]["cases_with_ssm_data"] is None
    assert row["assessment"]["necessary_count_order_check"] == "not_evaluable_missing_count"
    assert "candidate_availability_not_observed" in row["assessment"]["blockers"]
    assert validate_glioma_denominator_analysis(value) == value
    citation = next(item for item in value["citation_index"] if item["operation_id"] == "gdc.ssm_project_coverage")
    assert citation["pointer_authority"] == ("validated_missing_project_projection" if missing == "project" else "retained_response_native_payload")


def test_missing_gene_bucket_stays_unknown_even_when_native_parent_is_zero():
    native = _response()
    native["aggregations"]["projects"]["buckets"][0] = _bucket("TCGA-GBM", 0, missing_gene=True)
    native["hits"]["total"]["value"] = 20
    source = inputs()
    source["mutation_bundle"] = _execute(native).bundle
    value = assess_glioma_denominator(question=TASK, **source)
    assert value["projects"][0]["numerator"]["state"] == "gene_bucket_missing"
    assert value["projects"][0]["numerator"]["case_count_with_mutation"] is None
    assert "numerator_not_observed" in value["projects"][0]["assessment"]["blockers"]


def test_native_source_identity_and_clocks_remain_separate_without_asserting_same_release():
    value = assess_glioma_denominator(question=TASK, **inputs())
    context = value["acquisition_context"]
    assert context["mutation_retrieved_at"] != context["coverage_retrieved_at"]
    assert context["same_release"] == "not_established"
    assert context["release_identity_supplied"] is False
    assert value["projects"][0]["source_project_labels"]["exact_value_agreement"] is True
    assert value["projects"][0]["source_project_labels"]["scientific_population_equivalence"] == "unestablished"


def test_all_citations_replay_exact_retained_native_or_validated_projection_values():
    value = assess_glioma_denominator(question=TASK, **inputs())
    assert len(value["citation_index"]) == 4
    for citation in value["citation_index"]:
        key = "mutation_bundle" if citation["operation_id"] == "gdc.gene_mutation_counts" else "coverage_bundle"
        native = value["source_inputs"][key]
        for part in citation["artifact_pointer"].split("/")[1:]:
            native = native[int(part)] if isinstance(native, list) else native[part]
        assert native == citation["source_value"]
        assert content_digest(native) == citation["source_value_digest"]
        assert citation["source_bundle_digest"] == value["source_inputs"][key]["bundle_digest"]


@pytest.mark.parametrize("field", ["status", "projects", "acquisition_context", "denominator", "prevalence_calculated", "counts_pooled", "scientific_support_established", "clinical_actions_authorized", "citation_index", "missing_sources"])
def test_restamped_analysis_tampering_cannot_escape_native_replay(field):
    value = assess_glioma_denominator(question=TASK, **inputs())
    value[field] = "tampered"
    value["analysis_digest"] = content_digest({key: item for key, item in value.items() if key != "analysis_digest"})
    with pytest.raises(ArgumentError):
        validate_glioma_denominator_analysis(value)


@pytest.mark.parametrize("field", ["status", "human_review_required", "scientific_support_established", "clinical_actions_authorized", "citation_index"])
def test_restamped_handoff_cannot_release_scientific_hold(field):
    value = glioma_denominator_review_handoff(assess_glioma_denominator(question=TASK, **inputs()))
    value[field] = "tampered"
    value["handoff_digest"] = content_digest({key: item for key, item in value.items() if key != "handoff_digest"})
    with pytest.raises(ArgumentError):
        validate_glioma_denominator_handoff(value)


@pytest.mark.parametrize("question", [None, "", " ", "bad\x00text", "x"*8001])
def test_invalid_question_refuses_before_native_analysis(question):
    with pytest.raises(ArgumentError):
        assess_glioma_denominator(question=question)


def catalog_with_fixtures():
    calls, fixture = [], Fixture()
    native_mutation = _execute().bundle
    def mutation(_arguments, **_kwargs):
        calls.append(1)
        return WorkflowOperationResult(deepcopy(native_mutation), {"fixture": True})
    registered = register_gdc_gene_mutation_operations(WorkflowOperationCatalog())
    operation = registered.operations()[0]
    catalog = WorkflowOperationCatalog([operation], {operation.operation_id: mutation})
    register_gdc_ssm_coverage_operations(catalog, fetcher=fixture)
    register_glioma_denominator_workflow_operations(catalog)
    return catalog, calls, fixture


def test_exact_four_step_graph_requires_approval_pauses_resumes_and_replays_without_refetch():
    catalog, mutations, coverage = catalog_with_fixtures()
    plan = compile_glioma_denominator_workflow(TASK, catalog)
    assert [step["depends_on"] for step in plan.to_dict()["steps"][:2]] == [[], []]
    held = execute_workflow_plan(plan, catalog)
    assert held.status != "completed" and not mutations and not coverage.calls
    first = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1, retrieved_at=CLOCK)
    assert first.status == "paused" and len(mutations) == 1 and not coverage.calls
    second = execute_workflow_plan(plan, catalog, approved=True, checkpoint=first.checkpoint, max_step_calls=1, retrieved_at=CLOCK)
    assert second.status == "paused" and len(mutations) == 1 and len(coverage.calls) == 1
    complete = execute_workflow_plan(plan, catalog, approved=True, checkpoint=second.checkpoint, retrieved_at=CLOCK)
    assert complete.status == "completed" and len(mutations) == len(coverage.calls) == 1
    assert validate_glioma_denominator_handoff(complete.outputs["review"])["human_review_required"] is True
    replay = execute_workflow_plan(plan, catalog, approved=False, checkpoint=complete.checkpoint)
    assert replay.status == "completed" and len(mutations) == len(coverage.calls) == 1


@pytest.mark.parametrize("change", ["timeout", "subtree", "literal", "review_bypass", "question"])
def test_exact_task_contract_rejects_source_argument_native_root_and_review_substitutions(change):
    catalog, _, _ = catalog_with_fixtures()
    plan = compile_glioma_denominator_workflow(TASK, catalog).to_dict()
    proposal = {key: deepcopy(plan[key]) for key in ("name", "steps", "outputs")}
    if change == "timeout": proposal["steps"][1]["arguments"]["timeout_ms"] = 20_000
    elif change == "subtree": proposal["steps"][2]["arguments"]["mutation_bundle"] = {"$ref": "mutations#/projects"}
    elif change == "literal": proposal["steps"][2]["arguments"]["coverage_bundle"] = {}
    elif change == "review_bypass": proposal["outputs"] = ["analysis"]
    else: proposal["steps"][2]["arguments"]["question"] = "different"
    with pytest.raises(ArgumentError):
        compile_workflow_plan(proposal, catalog, task=TASK, requirements=plan["requirements"])


def test_duplicate_review_registration_refuses_without_partial_catalogue_mutation():
    catalog, _, _ = catalog_with_fixtures()
    review = next(row for row in catalog.operations() if row.operation_id == GLIOMA_DENOMINATOR_REVIEW_OPERATION)
    partial = WorkflowOperationCatalog([review], {review.operation_id: lambda *_args, **_kwargs: None})
    before = partial.to_dict()
    with pytest.raises(ArgumentError):
        register_glioma_denominator_workflow_operations(partial)
    assert partial.to_dict() == before
