"""Aggregate observations, missingness and review holds survive real SDK execution/replay."""
from copy import deepcopy
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.glioma_mutation_workflow import (
    GLIOMA_MUTATION_ANALYSIS_OPERATION, GLIOMA_MUTATION_REVIEW_OPERATION,
    analyze_glioma_mutation_counts, compile_glioma_mutation_workflow,
    glioma_mutation_review_handoff, register_glioma_mutation_workflow_operations,
    validate_glioma_mutation_analysis, validate_glioma_mutation_handoff,
)
from prism_sdk import reviewed_gdc_gene_mutation_retrieval as source
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import compile_workflow_requirements
from test_reviewed_gdc_gene_mutation_retrieval import _bucket, _execute, _response

TASK = "Review public TP53 mutation-affected case aggregates for TCGA-GBM and TCGA-LGG."


def _bundle(response=None):
    return _execute(response).bundle


def _catalog():
    catalog = source.register_gdc_gene_mutation_operations(WorkflowOperationCatalog())
    return register_glioma_mutation_workflow_operations(catalog)


def test_registration_refuses_a_late_duplicate_without_partially_mutating_the_catalogue():
    original = _catalog()
    review = next(operation for operation in original.operations() if operation.operation_id == GLIOMA_MUTATION_REVIEW_OPERATION)
    catalog = WorkflowOperationCatalog([review], {review.operation_id: lambda *_args, **_kwargs: None})
    before = catalog.to_dict()
    with pytest.raises(ValueError):
        register_glioma_mutation_workflow_operations(catalog)
    assert catalog.to_dict() == before


def test_observed_zero_retains_source_pointer_and_scientific_review_without_rate_or_pooling():
    analysis = analyze_glioma_mutation_counts(question=TASK, bundle=_bundle())
    assert analysis["observed_project_count"] == 2 and analysis["unobserved_project_count"] == 0
    assert [row["case_count_with_mutation"] for row in analysis["projects"]] == [10, 0]
    zero = analysis["projects"][1]
    assert zero["state"] == "observed" and zero["citation_id"] is not None
    assert zero["denominator"] is None and zero["prevalence"] is None
    assert zero["native_observation"]["source_json_pointer"].endswith("/doc_count")
    review = glioma_mutation_review_handoff(analysis)
    assert review["status"] == "awaiting_scientific_human_review"
    assert review["scientific_support_established"] is False and review["clinical_actions_authorized"] is False
    assert analysis["counts_pooled"] is False and analysis["cohort_overlap"] == "unassessed"
    assert len(review["citation_index"]) == 2 and review["analysis"] == analysis
    assert validate_glioma_mutation_handoff(review) == review


def test_citation_identity_binds_the_observation_instead_of_aliasing_changed_counts():
    first = analyze_glioma_mutation_counts(question=TASK, bundle=_bundle())
    changed = _response()
    changed["aggregations"]["projects"]["buckets"][0] = _bucket("TCGA-GBM", 11)
    changed["hits"]["total"]["value"] = 31
    second = analyze_glioma_mutation_counts(question=TASK, bundle=_bundle(changed))
    assert first["projects"][0]["case_count_with_mutation"] == 10
    assert second["projects"][0]["case_count_with_mutation"] == 11
    assert first["projects"][0]["citation_id"] != second["projects"][0]["citation_id"]
    assert first == analyze_glioma_mutation_counts(question=TASK, bundle=first["native_bundle"])


@pytest.mark.parametrize("change", ["status", "citation", "support", "clinical", "analysis", "extra"])
def test_restamped_handoff_cannot_replace_native_analysis_or_release_review(change):
    review = glioma_mutation_review_handoff(analyze_glioma_mutation_counts(question=TASK, bundle=_bundle()))
    if change == "status": review["status"] = "approved"
    elif change == "citation": review["citation_index"] = []
    elif change == "support": review["scientific_support_established"] = True
    elif change == "clinical": review["clinical_actions_authorized"] = True
    elif change == "analysis": review["analysis"]["projects"][0]["prevalence"] = 0.5
    else: review["invented"] = True
    review["handoff_digest"] = content_digest({key: value for key, value in review.items() if key != "handoff_digest"})
    with pytest.raises(ValueError):
        validate_glioma_mutation_handoff(review)


@pytest.mark.parametrize("missing", ["project", "gene", "all_requested_projects"])
def test_missing_native_counts_remain_unknown_instead_of_zero_or_negative_evidence(missing):
    response = _response()
    buckets = response["aggregations"]["projects"]["buckets"]
    if missing == "gene":
        buckets[0]["genes"]["my_genes"]["doc_count"] = 0
        buckets[0]["genes"]["my_genes"]["gene_id"]["buckets"] = []
    else:
        response["aggregations"]["projects"]["buckets"] = buckets[1:] if missing == "project" else buckets[2:]
    analysis = analyze_glioma_mutation_counts(question=TASK, bundle=_bundle(response))
    gbm = analysis["projects"][0]
    assert gbm["case_count_with_mutation"] is None and gbm["citation_id"] is None
    assert gbm["native_observation"]["source_json_pointer"] is None
    review = glioma_mutation_review_handoff(analysis)
    assert review["status"] == ("needs_evidence" if missing == "all_requested_projects" else "awaiting_scientific_human_review")
    assert analysis["status"] == "incomplete_aggregate_counts"


@pytest.mark.parametrize("change", ["count", "rate", "native_count", "citation", "support", "clinical", "drop_project", "extra", "bool_count"])
def test_restamped_derived_analysis_cannot_upgrade_or_change_native_observations(change):
    value = analyze_glioma_mutation_counts(question=TASK, bundle=_bundle())
    if change == "count": value["projects"][0]["case_count_with_mutation"] = 999
    elif change == "bool_count": value["projects"][1]["case_count_with_mutation"] = False
    elif change == "rate": value["projects"][0]["prevalence"] = 0.5
    elif change == "native_count": value["native_bundle"]["projects"][0]["case_count_with_mutation"] = 999
    elif change == "citation": value["citation_index"][0]["source_json_pointer"] = "/fabricated"
    elif change == "support": value["scientific_support_established"] = True
    elif change == "clinical": value["clinical_actions_authorized"] = True
    elif change == "drop_project": value["projects"].pop()
    else: value["invented_inference"] = "established"
    value["analysis_digest"] = content_digest({key: item for key, item in value.items() if key != "analysis_digest"})
    with pytest.raises(ValueError):
        validate_glioma_mutation_analysis(value)
    with pytest.raises(ValueError):
        glioma_mutation_review_handoff(value)


def test_native_source_is_replayed_and_inputs_are_not_mutated_or_aliasable():
    bundle = _bundle()
    original = deepcopy(bundle)
    analysis = analyze_glioma_mutation_counts(question=TASK, bundle=bundle)
    analysis["native_bundle"]["projects"][0]["case_count_with_mutation"] = 999
    assert bundle == original
    changed = deepcopy(bundle)
    changed["scientific_support_established"] = True
    changed["bundle_digest"] = content_digest({key: item for key, item in changed.items() if key != "bundle_digest"})
    with pytest.raises(ValueError):
        analyze_glioma_mutation_counts(question=TASK, bundle=changed)


@pytest.mark.parametrize("question", [None, False, "", " ", "bad\x00text", "x" * 8001, "\ud800"])
def test_invalid_research_label_refuses_before_native_bundle_processing(question):
    with pytest.raises(ValueError):
        analyze_glioma_mutation_counts(question=question, bundle={})


@pytest.mark.parametrize("target", ["source", "analysis"])
def test_exact_native_bundle_contract_rejects_child_pointer_before_any_execution(target):
    catalog = _catalog()
    plan = compile_glioma_mutation_workflow(TASK, catalog)
    assert plan.to_dict()["requirements"]["schema"] == "aurora-workflow-requirements/0.2"
    proposal = {key: plan.to_dict()[key] for key in ("name", "steps", "outputs")}
    step, key, producer = (1, "bundle", "counts") if target == "source" else (2, "analysis", "analysis")
    proposal["steps"][step]["arguments"][key] = {"$ref": producer + "#/projects"}
    with pytest.raises(ValueError):
        compile_workflow_plan(proposal, catalog, task=TASK, requirements=plan.to_dict()["requirements"])


def test_additional_contract_is_conjoined_and_cannot_silently_drop_the_review_output():
    catalog = _catalog()
    incompatible = compile_workflow_requirements(TASK, forbidden_operations=[GLIOMA_MUTATION_REVIEW_OPERATION])
    with pytest.raises(ValueError):
        compile_glioma_mutation_workflow(TASK, catalog, requirements=incompatible)


def test_serial_sdk_pause_resume_and_completed_replay_preserve_the_one_aggregate_read():
    original = _catalog()
    calls = []
    native = _execute()
    def read(arguments, **kwargs):
        calls.append(dict(arguments))
        assert kwargs["approve_source_dispatch"] is True
        return WorkflowOperationResult(native.bundle, native.receipt)
    def local(operation):
        return lambda arguments, **kwargs: original.execute(operation, arguments, **kwargs)
    executors = {operation.operation_id: read if operation.operation_id == "gdc.gene_mutation_counts" else local(operation.operation_id)
                 for operation in original.operations()}
    catalog = WorkflowOperationCatalog(original.operations(), executors)
    plan = compile_glioma_mutation_workflow(TASK, catalog)
    held = execute_workflow_plan(plan, catalog)
    assert held.status == "approval_required" and calls == []
    paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1)
    assert paused.status == "paused" and len(calls) == 1
    completed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint)
    assert completed.status == "completed" and len(calls) == 1
    assert completed.checkpoint["results"]["counts"] == paused.checkpoint["results"]["counts"]
    assert completed.outputs["review"]["status"] == "awaiting_scientific_human_review"
    assert completed.checkpoint["attempts"] == {"counts": 1, "analysis": 1, "review": 1}
    assert execute_workflow_plan(plan, catalog, checkpoint=completed.checkpoint).status == "completed"
    assert len(calls) == 1
    assert "native_bundle" not in json.dumps(completed.to_dict())
