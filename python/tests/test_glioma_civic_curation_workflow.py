"""Synthetic native curation fixtures; no live availability or scientific claims."""
import base64
import copy
import hashlib
import json
import socket

import pytest

from prism_sdk import glioma_civic_curation_workflow as curation
from prism_sdk import reviewed_civic_evidence_retrieval as civic
from prism_sdk.authoring import canonical_json, content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_integrations import WorkflowOperationCatalog

QUESTION = "Review supplied curation metadata."
NOW = "2026-10-27T19:00:00Z"
_CATALOG_EXECUTE = WorkflowOperationCatalog.execute


@pytest.fixture(autouse=True)
def deny_external_and_catalog_execution(monkeypatch):
    def denied(*_args, **_kwargs):
        pytest.fail("local curation fixture crossed a denied execution boundary")
    for owner, name in ((socket, "socket"), (socket, "getaddrinfo"), (LLMRuntime, "invoke"),
                        (LLMRuntime, "invoke_stream"), (LLMRuntime, "_post_once"),
                        (WorkflowOperationCatalog, "execute")):
        monkeypatch.setattr(owner, name, denied)


def native(total=1):
    return {"data": {"evidenceItems": {"totalCount": total,
        "pageInfo": {"hasNextPage": total > 25, "endCursor": "inert" if total else None},
        "nodes": [{"id": 100 + index, "status": "ACCEPTED", "flagged": False,
            "openRevisionCount": 0, "evidenceType": "DIAGNOSTIC", "evidenceLevel": "B",
            "evidenceDirection": "SUPPORTS", "evidenceRating": 3,
            "disease": {"id": 23, "doid": "3068", "name": "glioblastoma"},
            "source": {"id": 50, "citationId": "12345678", "displayType": "PubMed"}}
            for index in range(min(total, 25))]}}}


def bundle(payload=None, *, raw=None):
    raw = canonical_json(native() if payload is None else payload).encode("utf-8") if raw is None else raw
    def fetch(url, *, http_config):
        assert url == civic._URL
        return civic.CivicEvidenceHttpResponse(200, raw, url, len(raw), hashlib.sha256(raw).hexdigest(),
            http_config.transport_policy_digest, http_config.config_digest, "application/json")
    return civic.retrieve_civic_glioblastoma_evidence(approve_source_dispatch=True, retrieved_at=NOW, fetcher=fetch)


def analyze(source=None):
    return curation.analyze_glioma_civic_curation(question=QUESTION, civic_bundle=source)


def restamp(value, field="analysis_digest"):
    value[field] = content_digest({key: item for key, item in value.items() if key != field})
    return value


def test_source_absence_and_observed_empty_window_never_share_count_or_evidence_states():
    absent, empty = analyze(), analyze(bundle(native(0)))
    assert absent["status"] == "needs_evidence" and absent["source_state"] == "not_supplied"
    assert absent["source_coverage"] is None and absent["source_omissions"] is None
    assert absent["grouping_policy"]["observed_native_source_id_group_count"] is None
    assert absent["citation_index"] == [] and absent["connection_citations"] is None
    assert empty["source_state"] == "observed_empty_window" and empty["status"] == "curation_metadata_available_for_review"
    assert empty["source_coverage"]["returned_evidence_item_count"] == 0
    assert empty["grouping_policy"]["observed_native_source_id_group_count"] == 0
    nodes = next(row for row in empty["citation_index"] if row["record_kind"] == "native_evidence_item_array")
    assert nodes["source_value"] == []
    assert curation.glioma_civic_curation_review_handoff(absent)["status"] == "needs_evidence"
    assert curation.glioma_civic_curation_review_handoff(empty)["status"] == "awaiting_curation_metadata_research_review"


def test_every_native_citation_resolves_in_the_explicit_validated_bundle_namespace():
    source = bundle(native(2))
    result = analyze(source)
    assert result == analyze(source) == curation.validate_glioma_civic_curation_analysis(result)
    by_id = {row["citation_id"]: row for row in result["citation_index"]}
    assert len(by_id) == len(result["citation_index"]) == 6 + 2 * 17
    for citation in by_id.values():
        assert citation["artifact_pointer_root"] == "validated_civic_bundle"
        value = curation._pointer(source, citation["artifact_pointer"])
        assert canonical_json(value) == canonical_json(citation["source_value"])
        assert content_digest(value) == citation["source_value_digest"]
        assert citation["source_bundle_digest"] == source["bundle_digest"]
        assert citation["source_response_digest"] == source["source_inputs"][0]["response_digest"]
        assert citation["citation_id"] == "glioma_civic_curation:" + content_digest({key: value for key, value in citation.items() if key != "citation_id"})
    for item in result["item_observations"]:
        for field, link in item["observation_citations"].items():
            assert by_id[link["value_citation_id"]]["source_value"] == item["observations"][field]["value"]
            assert link["absence_context_citation_id"] is None
    assert result["source_inputs"]["civic_bundle"] == source
    assert result["source_inputs"]["civic_bundle"]["transport"]["mode"] == "caller_supplied"
    assert result["source_origin_authentication"] == "unestablished"


@pytest.mark.parametrize("field", ["disease", "evidenceRating", "doid", "endCursor"])
def test_missing_nullable_field_uses_containing_absence_context_while_null_has_native_value_citation(field):
    missing, null = native(), native()
    def target(payload):
        if field == "endCursor": return payload["data"]["evidenceItems"]["pageInfo"]
        row = payload["data"]["evidenceItems"]["nodes"][0]
        return row["disease"] if field == "doid" else row
    del target(missing)[field]
    target(null)[field] = None
    absent, explicit = analyze(bundle(missing)), analyze(bundle(null))
    def link(value):
        if field == "endCursor": return value["connection_citations"][field]
        item = value["item_observations"][0]
        return item["disease_field_citations"][field] if field == "doid" else item["observation_citations"][field]
    missing_link, null_link = link(absent), link(explicit)
    assert missing_link["value_citation_id"] is None and missing_link["absence_context_citation_id"] is not None
    assert null_link["value_citation_id"] is not None and null_link["absence_context_citation_id"] is None
    citation = next(row for row in absent["citation_index"] if row["citation_id"] == missing_link["absence_context_citation_id"])
    assert field not in citation["source_value"]
    citation = next(row for row in explicit["citation_index"] if row["citation_id"] == null_link["value_citation_id"])
    assert citation["source_value"] is None
    assert curation.validate_glioma_civic_curation_analysis(absent) == absent
    assert curation.validate_glioma_civic_curation_analysis(explicit) == explicit


def test_flags_and_revision_counts_are_independent_source_attention_without_sorting_or_quality_pass():
    payload = native(4)
    rows = payload["data"]["evidenceItems"]["nodes"]
    for row, flagged, revision in zip(rows, (False, True, False, True), (0, 0, 2, 3)):
        row.update(flagged=flagged, openRevisionCount=revision)
    result = analyze(bundle(payload))
    assert [row["evidence_id"] for row in result["item_observations"]] == [100, 101, 102, 103]
    assert [row["metadata_attention_reasons"] for row in result["item_observations"]] == [[], ["source_flagged"],
        ["source_open_revisions_reported"], ["source_flagged", "source_open_revisions_reported"]]
    assert all(row["attention_is_scientific_quality_ranking"] is False for row in result["item_observations"])
    assert result["scientific_support_established"] is False


@pytest.mark.parametrize("field,value", [("evidenceLevel", "A"), ("evidenceLevel", "E"),
    ("evidenceDirection", "SUPPORTS"), ("evidenceDirection", "DOES_NOT_SUPPORT"),
    ("evidenceDirection", "NA"), ("evidenceRating", 1), ("evidenceRating", 5)])
def test_curation_levels_directions_and_ratings_never_become_scientific_quality_or_clinical_support(field, value):
    payload = native()
    payload["data"]["evidenceItems"]["nodes"][0][field] = value
    result = analyze(bundle(payload))
    assert result["item_observations"][0]["observations"][field]["value"] == value
    assert result["scientific_support"] == result["clinical_support"] == "unestablished"
    assert result["scientific_support_established"] is False and result["clinical_actions_authorized"] is False
    assert result["independent_publication_count"] is None and result["patient_or_cohort_or_assay_denominator"] is None
    assert "quality_score" not in result["item_observations"][0]


def test_exact_source_id_groups_keep_all_members_and_conflicting_labels_without_publication_join():
    payload = native(4)
    rows = payload["data"]["evidenceItems"]["nodes"]
    rows[0]["source"] = {"id": 7, "citationId": "X", "displayType": "PubMed"}
    rows[1]["source"] = {"id": 7, "citationId": "x", "displayType": "PUBMED"}
    rows[2]["source"] = {"id": 8, "citationId": "X", "displayType": "PubMed"}
    rows[3]["source"] = dict(rows[0]["source"])
    result = analyze(bundle(payload))
    groups = result["native_source_id_groups"]
    assert [row["native_source_id"] for row in groups] == [7, 8]
    assert [[row["evidence_id"] for row in group["members"]] for group in groups] == [[100, 101, 103], [102]]
    assert groups[0]["source_metadata_state"] == "same_native_id_differing_metadata"
    assert groups[1]["source_metadata_state"] == "same_native_id_consistent_metadata"
    assert groups[0]["members"][1]["native_source"]["citationId"] == "x"
    assert len(result["item_observations"]) == 4 and result["grouping_policy"]["observed_native_source_id_group_count"] == 2
    assert result["grouping_policy"]["evidence_items_deduplicated"] is False
    assert result["grouping_policy"]["label_normalization_performed"] is False
    assert all(row["publication_identity_or_independence"] == "unestablished" for row in groups)


def test_page_metadata_original_omissions_and_question_label_remain_exact_without_another_request():
    source = bundle(native(27))
    question = "Inspect a different term; this text remains only a label."
    result = curation.analyze_glioma_civic_curation(question=question, civic_bundle=source)
    assert result["question"] == question and result["question_used_as_source_query"] is False
    assert result["source_coverage"] == source["coverage"] and result["source_omissions"] == source["omissions"]
    assert result["source_limitations"] == source["limitations"]
    assert result["source_coverage"]["unreturned_reported_evidence_item_count"] == 2
    assert result["source_coverage"]["pagination_followed"] is False
    assert result["source_coverage"]["completeness"] == result["source_coverage"]["omitted_influence"] == "unknown"
    assert source["receipts"][0]["source_request_count"] == 1
    assert all(row["omitted_count"] is None and row["influence"] == "unknown" for row in result["source_omissions"])


@pytest.mark.parametrize("field,value", [("flagged", None), ("openRevisionCount", None), ("status", "SUBMITTED")])
def test_invalid_required_native_fields_refuse_analysis_rather_than_becoming_missing_or_empty(field, value):
    source = bundle()
    payload = native()
    payload["data"]["evidenceItems"]["nodes"][0][field] = value
    source["source_inputs"][0]["response_body_base64"] = base64.b64encode(canonical_json(payload).encode()).decode()
    with pytest.raises(ArgumentError):
        analyze(restamp(source, "bundle_digest"))


@pytest.mark.parametrize("tamper", ["item_value", "attention", "groups", "source_count", "omission", "citation", "citation_link", "support", "extra_patient", "source_projection"])
def test_restamping_cannot_conceal_local_projection_or_boundary_disagreement_with_raw_bundle(tamper):
    result = analyze(bundle())
    if tamper == "item_value": result["item_observations"][0]["observations"]["evidenceRating"]["value"] = 5
    elif tamper == "attention": result["item_observations"][0]["metadata_attention_reasons"] = ["scientifically_validated"]
    elif tamper == "groups": result["native_source_id_groups"][0]["members"] = []
    elif tamper == "source_count": result["source_coverage"]["reported_filtered_total_count"] = 900
    elif tamper == "omission": result["source_omissions"][0]["influence"] = "zero"
    elif tamper == "citation": result["citation_index"][0]["source_value"] = {}
    elif tamper == "citation_link": result["item_observations"][0]["record_citation_id"] = "invented"
    elif tamper == "support": result["scientific_support_established"] = True
    elif tamper == "extra_patient": result["patient_records"] = []
    elif tamper == "source_projection":
        source = result["source_inputs"]["civic_bundle"]
        source["records"][0]["observations"]["flagged"]["value"] = True
        restamp(source, "bundle_digest")
    with pytest.raises(ArgumentError):
        curation.validate_glioma_civic_curation_analysis(restamp(result))


@pytest.mark.parametrize("tamper", ["status", "support", "review_complete", "citations", "embedded_analysis"])
def test_handoff_rebuild_rejects_restamped_upgrades_or_evidence_omissions(tamper):
    result = curation.glioma_civic_curation_review_handoff(analyze(bundle()))
    if tamper == "status": result["status"] = "approved"
    elif tamper == "support": result["clinical_support"] = "established"
    elif tamper == "review_complete": result["human_review_required"] = False
    elif tamper == "citations": result["citation_index"] = []
    elif tamper == "embedded_analysis":
        result["analysis"]["item_observations"] = []
        restamp(result["analysis"])
    with pytest.raises(ArgumentError):
        curation.validate_glioma_civic_curation_handoff(restamp(result, "handoff_digest"))


def maximal_source_bundle():
    payload = native(25)
    payload["data"]["evidenceItems"]["totalCount"] = 2_147_483_647
    payload["data"]["evidenceItems"]["pageInfo"] = {"hasNextPage": True, "endCursor": "c" * 4096}
    for index, row in enumerate(payload["data"]["evidenceItems"]["nodes"]):
        row.update(id=2_147_483_623 + index, flagged=True, openRevisionCount=2_147_483_647,
            evidenceType="PREDISPOSING", evidenceLevel="E", evidenceDirection="DOES_NOT_SUPPORT", evidenceRating=5)
        row["disease"] = {"id": 2_147_483_647, "doid": "d" * 256, "name": "n" * 4096}
        row["source"] = {"id": 2_147_483_623 + index, "citationId": "i" * 256, "displayType": "t" * 256}
    raw = canonical_json(payload).encode()
    assert len(raw) < civic.MAX_CIVIC_RESPONSE_BYTES
    return bundle(raw=raw + b" " * (civic.MAX_CIVIC_RESPONSE_BYTES - len(raw)))


def test_25_complete_rows_reach_exact_431_citation_bound_and_maximum_raw_source_bytes_remain_replayable():
    source = maximal_source_bundle()
    result = analyze(source)
    assert source["source_inputs"][0]["response_bytes"] == 524_288
    assert result["citation_policy"]["count"] == 6 + 25 * (1 + 10 + 3 + 3) == 431
    assert result["citation_policy"]["native_window_maximum"] == 431 <= 512
    assert len(result["item_observations"]) == 25 and len(result["native_source_id_groups"]) == 25
    assert result["citation_policy"]["omitted_local_citation_count"] == 0
    handoff = curation.glioma_civic_curation_review_handoff(result)
    assert curation.validate_glioma_civic_curation_handoff(handoff) == handoff
    assert len(canonical_json(result).encode()) <= 8_000_000
    assert len(canonical_json(handoff).encode()) <= 8_000_000


def test_citation_overflow_refuses_whole_analysis_without_a_partial_prefix(monkeypatch):
    source = bundle()
    monkeypatch.setattr(curation, "MAX_GLIOMA_CIVIC_CURATION_CITATIONS", 22)
    with pytest.raises(ArgumentError, match="overflow"):
        analyze(source)


@pytest.mark.parametrize("artifact", ["analysis", "handoff"])
def test_analysis_and_duplicated_handoff_have_independent_whole_artifact_byte_refusals(monkeypatch, artifact):
    source = bundle()
    result = analyze(source)
    handoff = curation.glioma_civic_curation_review_handoff(result)
    length = len(canonical_json(result if artifact == "analysis" else handoff).encode())
    if artifact == "handoff": assert length > len(canonical_json(result).encode())
    monkeypatch.setattr(curation, "MAX_GLIOMA_CIVIC_CURATION_BYTES", length - 1)
    with pytest.raises(ArgumentError):
        analyze(source) if artifact == "analysis" else curation.glioma_civic_curation_review_handoff(result)


def test_registration_is_inert_local_and_atomic_on_duplicate_or_capacity_failure(monkeypatch):
    catalog = curation.register_glioma_civic_curation_workflow_operations(WorkflowOperationCatalog())
    assert len(catalog.operations()) == 2 and all(row.implementation == "local" for row in catalog.operations())
    before = catalog.to_dict()
    with pytest.raises(ArgumentError, match="already registered"):
        curation.register_glioma_civic_curation_workflow_operations(catalog)
    assert catalog.to_dict() == before
    from prism_sdk import workflow_integrations
    monkeypatch.setattr(workflow_integrations, "MAX_WORKFLOW_OPERATIONS", 1)
    empty = WorkflowOperationCatalog()
    with pytest.raises(ArgumentError, match="capacity"):
        curation.register_glioma_civic_curation_workflow_operations(empty)
    assert not empty.operations()
