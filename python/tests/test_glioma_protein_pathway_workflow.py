"""Native replay, source partitions, unknown omissions and exact DAG contracts."""
from copy import deepcopy

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_protein_pathway_workflow import (
    analyze_glioma_protein_pathways, compile_glioma_protein_pathway_workflow,
    glioma_protein_pathway_review_handoff, register_glioma_protein_pathway_workflow_operations,
    validate_glioma_protein_pathway_analysis, validate_glioma_protein_pathway_handoff,
)
from prism_sdk.reviewed_reactome_retrieval import register_reactome_operations
from prism_sdk.reviewed_uniprot_retrieval import register_uniprot_operations, retrieve_uniprot_annotations
from prism_sdk.workflow_builder import execute_workflow_plan, compile_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from test_reviewed_reactome_retrieval import Fixture as ReactomeFixture, read as read_reactome, CLOCK
from test_reviewed_uniprot_retrieval import Fixture as UniProtFixture

TASK = "Inspect public glioma TP53 protein and pathway annotation context"


def inputs():
    return {"uniprot_bundle": retrieve_uniprot_annotations(fetcher=UniProtFixture(), approve_source_dispatch=True, retrieved_at=CLOCK),
        "reactome_bundle": read_reactome()}


def test_source_assertions_and_every_candidate_remain_under_scientific_review_hold():
    source = inputs()
    analysis = analyze_glioma_protein_pathways(question=TASK, **source)
    assert analysis["comparison"]["state"] == "exact_accession_value_agreement"
    assert analysis["comparison"]["scientific_identifier_equivalence"] == "unestablished"
    assert len(analysis["source_name_candidates"]) == len(source["uniprot_bundle"]["protein_names"])+len(source["uniprot_bundle"]["gene_name_candidates"])
    assert analysis["pathway_annotations"][0]["source_is_inferred"] is True
    assert analysis["pathway_annotations"][0]["glioma_activity"] == "unmeasured"
    assert analysis["measurements_joined"] is False
    assert validate_glioma_protein_pathway_analysis(analysis) == analysis
    handoff = glioma_protein_pathway_review_handoff(analysis)
    assert handoff["status"] == "awaiting_scientific_human_review"
    assert handoff["scientific_support_established"] is False
    assert validate_glioma_protein_pathway_handoff(handoff) == handoff


@pytest.mark.parametrize("missing", ["uniprot_bundle", "reactome_bundle", "both"])
def test_missing_source_is_unknown_and_needs_evidence_not_an_observed_empty(missing):
    source = inputs()
    for key in source:
        if key == missing or missing == "both":
            source[key] = None
    analysis = analyze_glioma_protein_pathways(question=TASK, **source)
    assert analysis["comparison"]["state"] == "not_comparable_missing_source"
    assert analysis["status"] == "needs_evidence"
    assert all(row["coverage"] is None for row in analysis["source_partitions"] if row["state"] == "not_supplied")
    assert glioma_protein_pathway_review_handoff(analysis)["status"] == "needs_evidence"


def test_empty_pathway_mapping_remains_observed_zero_without_negative_biology():
    source = inputs()
    source["reactome_bundle"] = read_reactome(ReactomeFixture([]))
    value = analyze_glioma_protein_pathways(question=TASK, **source)
    assert value["pathway_annotations"] == [] and value["missing_sources"] == []
    assert value["source_partitions"][1]["coverage"]["returned_pathway_count"] == 0
    assert any(row["record_kind"] == "observed_empty_pathway_array" and row["source_value"] == [] for row in value["citation_index"])
    assert value["scientific_support_established"] is False


def test_citation_cap_reports_exact_unknown_omissions_without_discarding_source_candidates():
    value = analyze_glioma_protein_pathways(question=TASK, max_citations=1, **inputs())
    policy = value["citation_policy"]
    assert len(value["citation_index"]) == 1
    assert policy["omitted_citation_count"] == policy["candidate_citation_count"]-1
    assert len(set(policy["omitted_citation_ids"])) == policy["omitted_citation_count"]
    assert policy["omitted_influence"] == "unknown"
    assert value["pathway_annotations"] and len(value["source_name_candidates"]) > 1
    assert validate_glioma_protein_pathway_analysis(value) == value


def test_every_retained_citation_points_to_exact_native_value_and_digest():
    value = analyze_glioma_protein_pathways(question=TASK, **inputs())
    for citation in value["citation_index"]:
        bundle = value["source_inputs"]["uniprot_bundle" if citation["operation_id"].startswith("uniprot") else "reactome_bundle"]
        native = bundle
        for token in citation["native_pointer"].split("/")[1:]:
            native = native[int(token)] if isinstance(native, list) else native[token]
        assert native == citation["source_value"]
        assert content_digest(native) == citation["source_value_digest"]


@pytest.mark.parametrize("field", ["comparison", "source_name_candidates", "pathway_annotations", "field_observations", "citation_index", "citation_policy", "status"])
def test_restamped_derived_analysis_changes_fail_replay(field):
    value = analyze_glioma_protein_pathways(question=TASK, **inputs())
    value[field] = "tampered"
    value["analysis_digest"] = content_digest({key: child for key, child in value.items() if key != "analysis_digest"})
    with pytest.raises(ArgumentError):
        validate_glioma_protein_pathway_analysis(value)


@pytest.mark.parametrize("cap", [True, 0, -1, 1025])
def test_invalid_citation_caps_refuse(cap):
    with pytest.raises(ArgumentError):
        analyze_glioma_protein_pathways(question=TASK, max_citations=cap)


def test_four_step_contract_keeps_sources_independent_and_resumes_completed_graph_without_calls():
    uniprot, reactome = UniProtFixture(), ReactomeFixture()
    catalog = register_uniprot_operations(WorkflowOperationCatalog(), fetcher=uniprot)
    register_reactome_operations(catalog, fetcher=reactome)
    register_glioma_protein_pathway_workflow_operations(catalog)
    plan = compile_glioma_protein_pathway_workflow(TASK, catalog)
    blocked = execute_workflow_plan(plan, catalog)
    assert blocked.status != "completed" and not uniprot.calls and not reactome.calls
    result = execute_workflow_plan(plan, catalog, approved=True, retrieved_at=CLOCK)
    assert result.status == "completed"
    assert len(uniprot.calls) == len(reactome.calls) == 1
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=result.checkpoint)
    assert resumed.status == "completed" and len(uniprot.calls) == len(reactome.calls) == 1
    assert result.outputs["review"]["human_review_required"] is True


@pytest.mark.parametrize("change", ["source_selector", "subtree", "literal_bundle", "output", "citation_cap", "question"])
def test_exact_template_contract_rejects_substitution_selector_changes_and_review_bypass(change):
    catalog = register_uniprot_operations(WorkflowOperationCatalog(), fetcher=UniProtFixture())
    register_reactome_operations(catalog, fetcher=ReactomeFixture())
    register_glioma_protein_pathway_workflow_operations(catalog)
    plan = compile_glioma_protein_pathway_workflow(TASK, catalog).to_dict()
    proposal = {key: deepcopy(plan[key]) for key in ("name", "steps", "outputs")}
    if change == "source_selector":
        proposal["steps"][0]["arguments"]["timeout_ms"] = 20000
    elif change == "subtree":
        proposal["steps"][2]["arguments"]["uniprot_bundle"] = {"$ref": "uniprot#/entry"}
    elif change == "literal_bundle":
        proposal["steps"][2]["arguments"]["reactome_bundle"] = {}
    elif change == "output":
        proposal["outputs"] = ["analysis"]
    elif change == "citation_cap":
        proposal["steps"][2]["arguments"]["max_citations"] = 1
    else:
        proposal["steps"][2]["arguments"]["question"] = "changed"
    with pytest.raises(ArgumentError):
        compile_workflow_plan(proposal, catalog, task=TASK, requirements=plan["requirements"])


def test_restamped_review_approval_does_not_survive_native_analysis_replay():
    value = glioma_protein_pathway_review_handoff(analyze_glioma_protein_pathways(question=TASK, **inputs()))
    value["human_review_required"] = False
    value["handoff_digest"] = content_digest({key: child for key, child in value.items() if key != "handoff_digest"})
    with pytest.raises(ArgumentError):
        validate_glioma_protein_pathway_handoff(value)
