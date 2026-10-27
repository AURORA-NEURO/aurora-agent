"""Exact source assertion comparison, contract-bound composition and scientific review holds."""
from copy import deepcopy
from dataclasses import replace
import json
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_gene_reference_workflow import (
    GLIOMA_GENE_REFERENCE_ANALYSIS_OPERATION as ANALYZE,
    GLIOMA_GENE_REFERENCE_REVIEW_OPERATION as REVIEW,
    analyze_glioma_gene_references, compile_glioma_gene_reference_workflow,
    glioma_gene_reference_review_handoff, glioma_gene_reference_workflow_proposal,
    register_glioma_gene_reference_workflow_operations,
    validate_glioma_gene_reference_analysis, validate_glioma_gene_reference_handoff,
)
from prism_sdk.reviewed_ensembl_gene_retrieval import (
    EnsemblGeneHttpResponse, register_ensembl_gene_operations, retrieve_ensembl_gene_references,
)
from prism_sdk.reviewed_ncbi_gene_retrieval import (
    ReviewedNcbiGeneRetrievalAdapter, ReviewedNcbiGeneRetrievalConfig, REVIEWED_NCBI_GENE_CATALOGUE,
)
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, WorkflowOperationResult, builtin_workflow_operation_catalog
from prism_sdk.workflow_requirements import compile_workflow_requirements


CLOCK = "2026-10-27T00:00:00Z"
TASK = "Review public glioma TP53 source gene references"
ENSG = "ENSG00000141510"


class EnsemblFixture:
    def __init__(self, ids=("7157",), fail=False):
        self.ids, self.fail, self.calls = ids, fail, []

    def __call__(self, url, **_kwargs):
        self.calls.append(url)
        if self.fail:
            raise OSError("fixture annotation source unavailable")
        value = ({"id": ENSG, "display_name": "TP53", "species": "homo_sapiens", "object_type": "Gene",
                  "db_type": "core", "version": 20, "assembly_name": "GRCh38", "source": None,
                  "biotype": "protein_coding", "description": "tumor protein p53"} if "/lookup/" in url else
                 [{"primary_id": identifier, "display_id": "TP53", "dbname": "EntrezGene", "version": "0",
                   "info_type": "DEPENDENT", "description": None} for identifier in self.ids])
        return EnsemblGeneHttpResponse(200, json.dumps(value).encode(), url, {"Content-Type": "application/json"})


def _ncbi_result(symbols=("TP53",), calls=None, fail=False, arguments=None):
    calls = [] if calls is None else calls
    config = ReviewedNcbiGeneRetrievalConfig(gene_symbols=symbols,
        transport_id="fixture.gene_reference_ncbi", transport_config_digest=content_digest({"fixture": "ncbi"}),
        timeout_ms=30_000 if arguments is None else arguments["timeout_ms"])
    def fetch(url):
        calls.append(url)
        if fail:
            raise OSError("fixture identity source unavailable")
        ids = [REVIEWED_NCBI_GENE_CATALOGUE[symbol] for symbol in config.gene_symbols]
        return {"header": {"type": "esummary", "version": "0.3"}, "result": {"uids": ids, **{REVIEWED_NCBI_GENE_CATALOGUE[symbol]: {
            "uid": REVIEWED_NCBI_GENE_CATALOGUE[symbol], "name": symbol, "description": symbol + " source identity",
            "chromosome": None, "maplocation": None, "otheraliases": "P53, 9999", "organism": {"taxid": 9606}}
            for symbol in config.gene_symbols}}}
    adapter = ReviewedNcbiGeneRetrievalAdapter(config, fetch=fetch)
    with patch("prism_sdk.reviewed_ncbi_gene_retrieval.acquire_ncbi_request_slot"):
        return adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=CLOCK)


def _inputs(ids=("7157",)):
    fixture = EnsemblFixture(ids)
    return (retrieve_ensembl_gene_references(approve_source_dispatch=True, retrieved_at=CLOCK, fetcher=fixture),
            _ncbi_result().bundle)


def _analyze(ids=("7157",)):
    ensembl, ncbi = _inputs(ids)
    return analyze_glioma_gene_references(question=TASK, ensembl_bundle=ensembl, ncbi_bundle=ncbi)


def _catalog(ids=("7157",), fail_ncbi=False):
    ensembl = EnsemblFixture(ids)
    ncbi_calls = []
    catalog = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=ensembl)
    descriptor = replace(builtin_workflow_operation_catalog().resolve("ncbi_gene.summary"),
                         description="Fixture-identified NCBI TP53 source reader", implementation="caller_adapter")
    def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
        assert approve_source_dispatch is True
        result = _ncbi_result(tuple(arguments["gene_symbols"]), calls=ncbi_calls, fail=fail_ncbi, arguments=arguments)
        return WorkflowOperationResult(result.bundle, result.receipt)
    catalog.register(descriptor, execute)
    register_glioma_gene_reference_workflow_operations(catalog)
    return catalog, ensembl, ncbi_calls


def _restamp_analysis(value):
    value["analysis_digest"] = content_digest({key: child for key, child in value.items() if key != "analysis_digest"})


def _restamp_source(value):
    value["source_set_digest"] = content_digest(value["sources"])
    value["bundle_digest"] = content_digest({key: child for key, child in value.items() if key != "bundle_digest"})


def test_exact_source_id_agreement_stays_scientifically_unestablished_and_every_source_value_is_cited():
    analysis = _analyze()
    assert analysis["comparison"]["state"] == "single_candidate_id_value_agreement"
    assert analysis["comparison"]["matching_candidate_count"] == 1
    assert analysis["comparison"]["mismatching_candidate_count"] == 0
    assert analysis["comparison"]["scientific_identifier_equivalence"] == "unestablished"
    assert analysis["candidate_comparisons"][0]["entrez_gene_id"] == "7157"
    assert analysis["candidate_comparisons"][0]["source_candidate"]["description"] is None
    assert len(analysis["citation_index"]) == 3
    for row in analysis["citation_index"]:
        assert row["source_value_digest"] == content_digest(row["source_value"])
        assert row["source_uri"].startswith("https://")
        assert row["native_pointer"].startswith("/")
        assert row["retrieved_at"] == CLOCK
    assert analysis["omitted_citation_count"] == 0
    assert analysis["measurements_joined"] is False
    assert analysis["cohorts_joined"] is False
    assert analysis["clinical_actions_authorized"] is False
    assert validate_glioma_gene_reference_analysis(analysis) == analysis


@pytest.mark.parametrize("ids,state,matching,mismatching", [
    ([], "observed_no_reference_candidates", 0, 0),
    (["9999"], "single_candidate_id_value_mismatch", 0, 1),
    (["7157", "9999"], "multiple_candidates_with_matching_id", 1, 1),
    (["9999", "1234"], "multiple_candidates_without_matching_id", 0, 2),
])
def test_empty_mismatching_and_multiple_candidates_remain_distinct_without_selecting_a_winner(ids, state, matching, mismatching):
    analysis = _analyze(ids)
    assert analysis["comparison"]["state"] == state
    assert analysis["comparison"]["matching_candidate_count"] == matching
    assert analysis["comparison"]["mismatching_candidate_count"] == mismatching
    assert [row["entrez_gene_id"] for row in analysis["candidate_comparisons"]] == ids
    assert analysis["comparison"]["multiple_candidates"] == (len(ids) > 1)
    assert analysis["missing_sources"] == []
    assert analysis["status"] == "source_assertions_ready_for_review"
    review = glioma_gene_reference_review_handoff(analysis)
    assert review["status"] == "awaiting_scientific_human_review"
    assert review["human_review_required"] and not review["scientific_support_established"]
    if not ids:
        empty_citation = next(row for row in analysis["citation_index"] if row["record_kind"] == "observed_empty_reference_array")
        assert empty_citation["source_value"] == []


@pytest.mark.parametrize("missing", ["ensembl", "ncbi", "both"])
def test_missing_sources_are_unknown_not_observed_zero_or_successful_empty_arrays(missing):
    ensembl, ncbi = _inputs()
    analysis = analyze_glioma_gene_references(question=TASK,
        ensembl_bundle=None if missing in {"ensembl", "both"} else ensembl,
        ncbi_bundle=None if missing in {"ncbi", "both"} else ncbi)
    assert analysis["status"] == "needs_evidence"
    assert analysis["comparison"]["state"] == "not_comparable_missing_source"
    assert analysis["comparison"]["matching_candidate_count"] is None
    assert analysis["comparison"]["mismatching_candidate_count"] is None
    for partition in analysis["source_partitions"]:
        if partition["state"] == "not_supplied":
            assert partition["returned_gene_count"] is None
            assert partition["returned_reference_count"] is None
            assert partition["coverage"] is None
    if missing == "ncbi":
        assert analysis["candidate_comparisons"][0]["comparison_state"] == "not_compared_missing_ncbi"
        assert analysis["candidate_comparisons"][0]["ncbi_expected_entrez_id"] is None
    assert glioma_gene_reference_review_handoff(analysis)["status"] == "needs_evidence"
    validate_glioma_gene_reference_analysis(analysis)


@pytest.mark.parametrize("bad", [{}, {"status": "failed"}, [], False, 0, "not supplied"])
@pytest.mark.parametrize("side", ["ensembl_bundle", "ncbi_bundle"])
def test_failed_malformed_or_false_source_values_never_become_missing_or_empty_success(side, bad):
    with pytest.raises(ValueError):
        analyze_glioma_gene_references(question=TASK, **{side: bad})


def test_no_alias_case_or_numeric_coercion_can_supply_an_identifier_match():
    analysis = _analyze(["9999"])
    assert "9999" in analysis["source_inputs"]["ncbi_bundle"]["genes"][0]["aliases"]
    assert analysis["comparison"]["matching_candidate_count"] == 0
    assert analysis["comparison"]["alias_matching_performed"] is False
    assert analysis["comparison"]["case_folding_performed"] is False
    assert analysis["comparison"]["numeric_coercion_performed"] is False
    for mutation in (7157, "07157"):
        ensembl, ncbi = _inputs()
        row = ensembl["source_inputs"][1]["native_payload"][0]
        row["primary_id"] = mutation
        ensembl["source_inputs"][1]["payload_digest"] = content_digest(ensembl["source_inputs"][1]["native_payload"])
        ensembl["bundle_digest"] = content_digest({key: value for key, value in ensembl.items() if key != "bundle_digest"})
        with pytest.raises(ValueError):
            analyze_glioma_gene_references(question=TASK, ensembl_bundle=ensembl, ncbi_bundle=ncbi)


def test_ncbi_scope_is_exactly_one_human_TP53_identity_and_not_a_selected_alias_from_a_larger_bundle():
    ensembl, _ = _inputs()
    for symbols in (("IDH1",), ("TP53", "IDH1")):
        with pytest.raises(ValueError, match="TP53"):
            analyze_glioma_gene_references(question=TASK, ensembl_bundle=ensembl,
                                          ncbi_bundle=_ncbi_result(symbols).bundle)


@pytest.mark.parametrize("edit", [
    lambda value: value["genes"][0].update(gene_id=7157),
    lambda value: value["genes"][0].update(symbol="tp53"),
    lambda value: value["genes"][0].update(organism_taxid=True),
    lambda value: value.update(gene_count=0),
    lambda value: value["sources"][0].update(uri="https://example.org/identity"),
    lambda value: value["genes"][0].update(patient_id="not admitted"),
])
def test_native_ncbi_normalization_and_provenance_replay_reject_restamped_mutation(edit):
    ensembl, ncbi = _inputs()
    edit(ncbi)
    _restamp_source(ncbi)
    with pytest.raises(ValueError):
        analyze_glioma_gene_references(question=TASK, ensembl_bundle=ensembl, ncbi_bundle=ncbi)


@pytest.mark.parametrize("edit", [
    lambda value: value["comparison"].update(scientific_identifier_equivalence="verified"),
    lambda value: value["comparison"].update(matching_candidate_count=0),
    lambda value: value["candidate_comparisons"][0].update(entrez_gene_id="9999"),
    lambda value: value["candidate_comparisons"][0].update(comparison_state="id_value_mismatch"),
    lambda value: value["citation_index"][0].update(native_pointer="/gene"),
    lambda value: value["citation_index"][1].update(source_value={"primary_id": "9999"}),
    lambda value: value["source_partitions"][0]["omissions"].update(reference_exhaustiveness="complete"),
    lambda value: value.update(measurements_joined=True),
    lambda value: value.update(human_review_required=False),
    lambda value: value.update(omitted_citation_count=1),
    lambda value: value.update(extra_field="not admitted"),
])
def test_recomputed_outer_analysis_digest_does_not_bypass_full_derived_replay(edit):
    analysis = _analyze()
    edit(analysis)
    _restamp_analysis(analysis)
    with pytest.raises(ValueError):
        validate_glioma_gene_reference_analysis(analysis)


def test_review_handoff_is_defensive_and_remains_a_hold_after_source_agreement():
    analysis = _analyze()
    handoff = glioma_gene_reference_review_handoff(analysis)
    assert validate_glioma_gene_reference_handoff(handoff) == handoff
    assert handoff["scientific_identifier_equivalence"] == "unestablished"
    assert handoff["clinical_actions_authorized"] is False
    for field, value in (("human_review_required", False), ("status", "approved"),
                         ("scientific_identifier_equivalence", "proven")):
        changed = deepcopy(handoff)
        changed[field] = value
        changed["handoff_digest"] = content_digest({key: child for key, child in changed.items() if key != "handoff_digest"})
        with pytest.raises(ValueError):
            validate_glioma_gene_reference_handoff(changed)
    checked = validate_glioma_gene_reference_analysis(analysis)
    checked["comparison"]["state"] = "changed"
    assert analysis["comparison"]["state"] == "single_candidate_id_value_agreement"


@pytest.mark.parametrize("question", ["", " ", "a" * 8001, "TP53\x00question", False])
def test_question_is_bounded_label_and_cannot_change_fixed_source_selectors(question):
    with pytest.raises(ValueError):
        analyze_glioma_gene_references(question=question)
    with pytest.raises(ValueError):
        glioma_gene_reference_workflow_proposal(question)


def test_local_registration_is_inert_and_never_claims_new_external_services():
    catalog = WorkflowOperationCatalog()
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval._default_fetch") as fetch:
        register_glioma_gene_reference_workflow_operations(catalog)
        result = catalog.execute(ANALYZE, {"question": TASK, "ensembl_bundle": None, "ncbi_bundle": None})
        review = catalog.execute(REVIEW, {"analysis": result.value})
    fetch.assert_not_called()
    assert len(catalog.operations()) == 2
    assert all(operation.implementation == "local" for operation in catalog.operations())
    assert {operation.integration_id for operation in catalog.operations()} == {"local.glioma_gene_references"}
    assert review.value["status"] == "needs_evidence"


def test_compiler_binds_schema_02_whole_source_bundles_exact_arguments_review_output_and_call_budgets():
    catalog, ensembl, ncbi_calls = _catalog()
    plan = compile_glioma_gene_reference_workflow(TASK, catalog)
    requirements = plan.to_dict()["requirements"]
    assert requirements["schema"].endswith("/0.2")
    assert requirements["max_steps"] == 4
    assert requirements["output_operations"] == [REVIEW]
    assert all(value == 1 for value in requirements["max_calls_by_operation"].values())
    assert len(requirements["data_links"]) == 3
    assert all(link["source_pointer"] == "" for link in requirements["data_links"])
    assert ensembl.calls == [] and ncbi_calls == []
    for edit in (
        lambda proposal: proposal["steps"][0]["arguments"].update(timeout_ms=20000),
        lambda proposal: proposal["steps"][1]["arguments"].update(gene_symbols=["IDH1"]),
        lambda proposal: proposal["steps"][2]["arguments"].update(ensembl_bundle={"$ref": "ensembl#/gene"}),
        lambda proposal: proposal["steps"][2]["arguments"].update(ncbi_bundle={"$ref": "ncbi#/genes"}),
        lambda proposal: proposal["steps"][3]["arguments"].update(analysis={"$ref": "analysis#/comparison"}),
        lambda proposal: proposal.update(outputs=["analysis"]),
    ):
        proposal = glioma_gene_reference_workflow_proposal(TASK)
        edit(proposal)
        with pytest.raises(ValueError):
            compile_workflow_plan(proposal, catalog, task=TASK, requirements=requirements)


def test_caller_requirements_are_conjoined_and_conflicting_selectors_refuse_before_source_dispatch():
    catalog, ensembl, ncbi_calls = _catalog()
    compatible = compile_workflow_requirements(TASK, max_steps=4,
        output_operations=[REVIEW], max_calls_by_operation={"ensembl.gene_references": 1})
    plan = compile_glioma_gene_reference_workflow(TASK, catalog, requirements=compatible)
    assert plan.to_dict()["requirements"]["max_steps"] == 4
    for contract in (compile_workflow_requirements(TASK, max_steps=3),
                     compile_workflow_requirements(TASK, forbidden_operations=["ensembl.gene_references"]),
                     compile_workflow_requirements(TASK, argument_rules=[{"operation": "ncbi_gene.summary", "path": "",
                         "comparison": "equals", "value": {"gene_symbols": ["IDH1"], "timeout_ms": 30000}}])):
        with pytest.raises(ValueError):
            compile_glioma_gene_reference_workflow(TASK, catalog, requirements=contract)
    assert ensembl.calls == [] and ncbi_calls == []


def test_four_step_workflow_pauses_after_sources_resumes_locally_and_replays_with_zero_further_source_calls():
    catalog, ensembl, ncbi_calls = _catalog()
    plan = compile_glioma_gene_reference_workflow(TASK, catalog)
    denied = execute_workflow_plan(plan, catalog)
    assert denied.status == "approval_required"
    assert ensembl.calls == [] and ncbi_calls == []
    paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=2, retrieved_at=CLOCK)
    assert paused.status == "paused" and len(ensembl.calls) == 2 and len(ncbi_calls) == 1
    completed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint, retrieved_at=CLOCK)
    assert completed.status == "completed"
    assert completed.checkpoint["results"]["ensembl"] == paused.checkpoint["results"]["ensembl"]
    assert completed.checkpoint["results"]["ncbi"] == paused.checkpoint["results"]["ncbi"]
    handoff = validate_glioma_gene_reference_handoff(completed.outputs["review"])
    assert handoff["status"] == "awaiting_scientific_human_review"
    replay = execute_workflow_plan(plan, catalog, approved=True, checkpoint=completed.checkpoint, retrieved_at=CLOCK)
    assert replay.status == "completed" and len(ensembl.calls) == 2 and len(ncbi_calls) == 1


def test_failed_source_halts_before_local_comparison_and_never_becomes_a_missing_bundle_artifact():
    catalog, ensembl, ncbi_calls = _catalog(fail_ncbi=True)
    plan = compile_glioma_gene_reference_workflow(TASK, catalog)
    run = execute_workflow_plan(plan, catalog, approved=True, retrieved_at=CLOCK)
    assert run.status == "failed"
    assert "analysis" not in run.checkpoint["results"] and "review" not in run.outputs
    assert len(ensembl.calls) == 2 and len(ncbi_calls) == 1
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=run.checkpoint, retrieved_at=CLOCK)
    assert resumed.status == "retry_required"
    assert len(ensembl.calls) == 2 and len(ncbi_calls) == 1


def test_oversized_artifacts_and_unbounded_nested_inputs_are_refused_before_analysis():
    ensembl, ncbi = _inputs()
    ensembl["extra"] = "x" * 2_000_001
    with pytest.raises(ValueError, match="byte bound"):
        analyze_glioma_gene_references(question=TASK, ensembl_bundle=ensembl, ncbi_bundle=ncbi)
    with pytest.raises(ValueError):
        validate_glioma_gene_reference_analysis({"nested": float("nan")})


def test_local_registration_capacity_failure_is_atomic_before_first_operation():
    catalog = WorkflowOperationCatalog()
    with patch("prism_sdk.workflow_integrations.MAX_WORKFLOW_OPERATIONS", 1):
        with pytest.raises(ValueError, match="capacity"):
            register_glioma_gene_reference_workflow_operations(catalog)
    assert catalog.operations() == ()


def test_full_64_candidate_boundary_is_preserved_and_none_is_promoted_to_equivalence():
    ids = [str(index + 1) for index in range(63)] + ["7157"]
    analysis = _analyze(ids)
    assert len(analysis["candidate_comparisons"]) == 64
    assert analysis["comparison"]["matching_candidate_count"] == 1
    assert analysis["comparison"]["mismatching_candidate_count"] == 63
    assert len(analysis["citation_index"]) == 66 and analysis["omitted_citation_count"] == 0
    assert analysis["comparison"]["state"] == "multiple_candidates_with_matching_id"
    assert all(row["scientific_identifier_equivalence"] == "unestablished" for row in analysis["candidate_comparisons"])
    validate_glioma_gene_reference_handoff(glioma_gene_reference_review_handoff(analysis))
