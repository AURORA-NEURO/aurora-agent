"""Every HGNC/Ensembl/NCBI assertion survives without choosing scientific equivalence."""
from copy import deepcopy

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_gene_identity_workflow import (
    GLIOMA_GENE_IDENTITY_ANALYSIS_OPERATION, GLIOMA_GENE_IDENTITY_REVIEW_OPERATION,
    analyze_glioma_gene_identity, compile_glioma_gene_identity_workflow, glioma_gene_identity_review_handoff,
    register_glioma_gene_identity_workflow_operations, validate_glioma_gene_identity_analysis,
    validate_glioma_gene_identity_handoff,
)
from prism_sdk.reviewed_hgnc_gene_retrieval import register_hgnc_gene_operations
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog
from test_reviewed_hgnc_gene_retrieval import Fixture, read, gene, payload, CLOCK
from test_glioma_gene_reference_workflow import _inputs, _catalog

TASK = "Review fixed TP53 HGNC, Ensembl and NCBI source identity metadata for glioma research."


def inputs(ids=("7157",)):
    ensembl, ncbi = _inputs(ids)
    return {"hgnc_bundle": read(), "ensembl_bundle": ensembl, "ncbi_bundle": ncbi}


def test_all_three_sources_and_exact_metadata_concordance_remain_under_scientific_hold():
    value = analyze_glioma_gene_identity(question=TASK, **inputs())
    assert value["status"] == "source_assertions_ready_for_review"
    assert len(value["source_partitions"]) == 3
    row = value["hgnc_candidate_observations"][0]
    assert row["ensembl_gene_id_comparison"]["state"] == row["ncbi_entrez_id_comparison"]["state"] == "exact_string_agreement"
    assert row["field_observations"]["alias_symbol"]["value"] == ["P53", "BCC7", "P53"]
    assert row["scientific_identifier_equivalence"] == "unestablished"
    assert value["comparison_policy"]["preferred_identity_selected"] is False
    handoff = glioma_gene_identity_review_handoff(value)
    assert handoff["status"] == "awaiting_scientific_human_review"
    assert handoff["scientific_support_established"] is False and handoff["clinical_actions_authorized"] is False
    assert validate_glioma_gene_identity_handoff(handoff) == handoff


def test_every_ensembl_reference_candidate_is_compared_without_choosing_matching_one():
    value = analyze_glioma_gene_identity(question=TASK, **inputs(("7157", "999")))
    checks = value["hgnc_candidate_observations"][0]["all_ensembl_entrez_candidate_comparisons"]
    assert [row["ensembl_entrez_candidate"] for row in checks] == ["7157", "999"]
    assert [row["state"] for row in checks] == ["exact_string_agreement", "string_mismatch"]
    assert value["status"] == "metadata_discrepancy_for_review"
    assert value["comparison_policy"]["scientific_identifier_equivalence"] == "unestablished"


@pytest.mark.parametrize("field,value", [("ensembl_gene_id", "ENSG00000100000"), ("entrez_id", "999")])
def test_disagreeing_hgnc_reference_is_retained_as_metadata_discrepancy(field, value):
    source = inputs(); source["hgnc_bundle"] = read(Fixture(payload([{**gene(), field:value}])))
    analysis = analyze_glioma_gene_identity(question=TASK, **source)
    assert analysis["status"] == "metadata_discrepancy_for_review"
    assert analysis["hgnc_candidate_observations"][0]["field_observations"][field]["value"] == value


@pytest.mark.parametrize("state", ["absent", "withdrawn", "multiple", "missing_status", "partial", "approximate"])
def test_nonready_hgnc_state_cannot_promote_existing_two_source_agreement(state):
    native = payload()
    if state == "absent": native = payload([])
    elif state == "withdrawn": native["response"]["docs"][0]["status"] = "Entry Withdrawn"
    elif state == "multiple": native = payload([gene(), {**gene(), "hgnc_id":"HGNC:999"}])
    elif state == "missing_status": native["response"]["docs"][0].pop("status")
    elif state == "partial": native["response"]["numFound"] = 3
    else: native["response"]["numFoundExact"] = False
    source = inputs(); source["hgnc_bundle"] = read(Fixture(native))
    value = analyze_glioma_gene_identity(question=TASK, **source)
    assert value["status"] == "needs_identity_resolution"
    assert glioma_gene_identity_review_handoff(value)["status"] == "needs_identity_resolution"
    assert value["ensembl_ncbi_context"]["comparison"]["state"] == "single_candidate_id_value_agreement"
    assert len(value["hgnc_candidate_observations"]) == len(native["response"]["docs"])


@pytest.mark.parametrize("field,kind", [("entrez_id","missing"),("entrez_id","null"),("entrez_id","empty"),
    ("ensembl_gene_id","missing"),("ensembl_gene_id","null"),("ensembl_gene_id","empty")])
def test_missing_null_empty_hgnc_identifier_stays_unobserved_without_alias_or_numeric_fallback(field, kind):
    row=gene()
    if kind == "missing": row.pop(field)
    else: row[field] = None if kind == "null" else ""
    source=inputs(); source["hgnc_bundle"]=read(Fixture(payload([row])))
    value=analyze_glioma_gene_identity(question=TASK,**source)
    assert value["status"] == "needs_evidence"
    assert value["hgnc_candidate_observations"][0]["field_observations"][field]["state"] == kind
    assert value["comparison_policy"]["alias_matching_performed"] is False
    assert value["comparison_policy"]["numeric_coercion_performed"] is False


@pytest.mark.parametrize("missing", ["hgnc_bundle","ensembl_bundle","ncbi_bundle","all"])
def test_missing_acquisition_is_unknown_without_replacing_observed_empty_records(missing):
    source=inputs()
    for key in source:
        if key == missing or missing == "all": source[key] = None
    value=analyze_glioma_gene_identity(question=TASK,**source)
    assert value["status"] == "needs_evidence" and value["missing_sources"]
    assert all(row["coverage"] is None for row in value["source_partitions"] if row["state"] == "not_supplied")
    assert validate_glioma_gene_identity_analysis(value) == value


def test_every_citation_resolves_to_its_exact_retained_native_value():
    value=analyze_glioma_gene_identity(question=TASK,**inputs())
    names={"hgnc.gene_references":"hgnc_bundle","ensembl.gene_references":"ensembl_bundle","ncbi_gene.summary":"ncbi_bundle"}
    for citation in value["citation_index"]:
        target=value["source_inputs"][names[citation["operation_id"]]]
        for token in citation["native_pointer"].split("/")[1:]:
            target=target[int(token)] if isinstance(target,list) else target[token]
        assert target == citation["source_value"]
        assert content_digest(target) == citation["source_value_digest"]


def test_citation_cap_preserves_exact_unknown_omissions_without_dropping_any_native_candidate():
    value=analyze_glioma_gene_identity(question=TASK,max_citations=1,**inputs(("7157","999")))
    policy=value["citation_policy"]
    assert len(value["citation_index"]) == 1 and policy["omitted_citation_count"] == policy["candidate_citation_count"]-1
    assert len(policy["omitted_citation_ids"]) == policy["omitted_citation_count"]
    assert policy["omitted_influence"] == "unknown"
    assert len(value["hgnc_candidate_observations"][0]["all_ensembl_entrez_candidate_comparisons"]) == 2
    assert validate_glioma_gene_identity_analysis(value) == value


@pytest.mark.parametrize("field", ["status","source_partitions","missing_sources","hgnc_candidate_observations","comparison_policy","citation_index","citation_policy","scientific_support_established","clinical_actions_authorized"])
def test_restamped_derived_analysis_changes_fail_full_native_replay(field):
    value=analyze_glioma_gene_identity(question=TASK,**inputs()); value[field]="tampered"
    value["analysis_digest"]=content_digest({key:item for key,item in value.items() if key != "analysis_digest"})
    with pytest.raises(ArgumentError): validate_glioma_gene_identity_analysis(value)


def test_restamped_handoff_cannot_release_the_scientific_review_hold():
    value=glioma_gene_identity_review_handoff(analyze_glioma_gene_identity(question=TASK,**inputs()))
    value["human_review_required"]=False
    value["handoff_digest"]=content_digest({key:item for key,item in value.items() if key != "handoff_digest"})
    with pytest.raises(ArgumentError): validate_glioma_gene_identity_handoff(value)


def catalog():
    result,ensembl,ncbi=_catalog(); hgnc=Fixture()
    register_hgnc_gene_operations(result,fetcher=hgnc)
    register_glioma_gene_identity_workflow_operations(result)
    return result,hgnc,ensembl,ncbi


def test_five_step_native_graph_pauses_after_sources_and_replays_completed_plan_without_calls():
    installed,hgnc,ensembl,ncbi=catalog()
    plan=compile_glioma_gene_identity_workflow(TASK,installed)
    assert len(plan.to_dict()["steps"]) == 5
    assert all(step["depends_on"] == [] for step in plan.to_dict()["steps"][:3])
    held=execute_workflow_plan(plan,installed)
    assert held.status == "approval_required" and not hgnc.calls and not ensembl.calls and not ncbi
    sources=execute_workflow_plan(plan,installed,approved=True,max_step_calls=3,retrieved_at=CLOCK)
    assert sources.status == "paused" and len(hgnc.calls) == len(ncbi) == 1 and len(ensembl.calls) == 2
    complete=execute_workflow_plan(plan,installed,approved=True,checkpoint=sources.checkpoint,retrieved_at=CLOCK)
    assert complete.status == "completed"
    assert validate_glioma_gene_identity_handoff(complete.outputs["review"])["human_review_required"] is True
    replay=execute_workflow_plan(plan,installed,approved=False,checkpoint=complete.checkpoint)
    assert replay.status == "completed" and len(hgnc.calls) == len(ncbi) == 1 and len(ensembl.calls) == 2


@pytest.mark.parametrize("change", ["selector","subtree","literal","output","question","citation_cap"])
def test_exact_template_contract_refuses_selector_native_root_and_review_substitutions(change):
    installed,*_=catalog(); plan=compile_glioma_gene_identity_workflow(TASK,installed).to_dict()
    proposal={key:deepcopy(plan[key]) for key in ("name","steps","outputs")}
    if change == "selector": proposal["steps"][2]["arguments"]["gene_symbols"]=["EGFR"]
    elif change == "subtree": proposal["steps"][3]["arguments"]["hgnc_bundle"]={"$ref":"hgnc#/records"}
    elif change == "literal": proposal["steps"][3]["arguments"]["ncbi_bundle"]={}
    elif change == "output": proposal["outputs"]=["analysis"]
    elif change == "question": proposal["steps"][3]["arguments"]["question"]="different"
    else: proposal["steps"][3]["arguments"]["max_citations"]=1
    with pytest.raises(ArgumentError): compile_workflow_plan(proposal,installed,task=TASK,requirements=plan["requirements"])


@pytest.mark.parametrize("cap", [True,0,1025])
def test_unbounded_or_nontyped_citation_caps_refuse(cap):
    with pytest.raises(ArgumentError): analyze_glioma_gene_identity(question=TASK,max_citations=cap)


@pytest.mark.parametrize("tag", ["Ambiguous", "Ambigous", "Placeholder symbol", "Future source tag"])
def test_explicit_hgnc_symbol_tag_requires_resolution_despite_matching_reference_strings(tag):
    source = inputs()
    source["hgnc_bundle"] = read(Fixture(payload([{**gene(), "symbol_report_tag": tag}])))
    value = analyze_glioma_gene_identity(question=TASK, **source)
    assert value["status"] == "needs_identity_resolution"
    assert value["hgnc_candidate_observations"][0]["ncbi_entrez_id_comparison"]["state"] == "exact_string_agreement"
    assert glioma_gene_identity_review_handoff(value)["status"] == "needs_identity_resolution"
