"""Offline admission, missingness, retained bytes and source transport invariants."""
from copy import deepcopy
import hashlib
import json
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.reviewed_public_http import ReviewedPublicHttpError
from prism_sdk import reviewed_gdc_ssm_coverage_retrieval as source
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, builtin_workflow_operation_catalog

CLOCK = "2026-10-27T12:00:00Z"


def bucket(project="TCGA-GBM", total=20, associated=15):
    return {"key": project, "doc_count": total,
        "case_summary": {"doc_count": total*3, "case_with_ssm": {"doc_count": associated}}}


def payload():
    return {"took": 2, "timed_out": False,
        "_shards": {"total": 12, "successful": 12, "skipped": 0, "failed": 0},
        "hits": {"total": {"value": 34, "relation": "eq"}, "max_score": None, "hits": []},
        "aggregations": {"projects": {"doc_count_error_upper_bound": 0, "sum_other_doc_count": 0,
            "buckets": [bucket(), bucket("TCGA-LGG", 10, 0), bucket("OTHER", 4, 3)]}}}


class Fixture:
    def __init__(self, value=None, *, body=None):
        self.value = payload() if value is None else value
        self.body, self.calls = body, []

    def __call__(self, url, *, http_config):
        self.calls.append((url, http_config))
        body = json.dumps(self.value).encode() if self.body is None else self.body
        return source.GdcSsmCoverageHttpResponse(200, body, url, len(body), hashlib.sha256(body).hexdigest(),
            http_config.transport_policy_digest, http_config.config_digest, "application/json")


def read(fixture=None):
    return source.retrieve_gdc_ssm_coverage(approve_source_dispatch=True, retrieved_at=CLOCK,
        fetcher=Fixture() if fixture is None else fixture)


def restamp(value):
    value["bundle_digest"] = content_digest({key: item for key, item in value.items() if key != "bundle_digest"})
    return value


def test_fixed_aggregate_route_retains_all_public_native_buckets_and_distinct_counts():
    fixture = Fixture()
    value = read(fixture)
    assert len(fixture.calls) == 1
    assert fixture.calls[0][0] == "https://api.gdc.cancer.gov/analysis/mutated_cases_count_by_project?size=0"
    assert fixture.calls[0][1].max_response_bytes == 1_000_000
    assert value["source_inputs"][0]["native_payload"] == payload()
    assert value["projects"][0]["project_doc_count"] == 20
    assert value["projects"][0]["case_summary_doc_count"] == 60
    assert value["projects"][0]["cases_with_ssm_data"] == 15
    assert value["coverage"]["unanalysed_project_count"] == 1
    assert value["coverage"]["unanalysed_influence"] == "unknown"
    assert source.validate_gdc_ssm_coverage_bundle(value) == value


def test_observed_zero_is_not_a_missing_count_or_zero_assay_denominator():
    value = read()
    row = value["projects"][1]
    assert row["state"] == "observed" and row["cases_with_ssm_data"] == 0
    assert row["native_ssm_count_pointer"].endswith("/case_with_ssm/doc_count")
    assert row["gene_specific_assay_denominator"] is None
    assert row["denominator_state"] == "not_established"
    assert value["coverage"]["prevalence_calculated"] is False


@pytest.mark.parametrize("missing,expected", [("project", "project_bucket_missing"),
    ("summary", "case_summary_missing"), ("ssm_bucket", "ssm_bucket_missing"), ("ssm_count", "ssm_count_missing")])
def test_missing_native_container_and_count_states_remain_unknown(missing, expected):
    native = payload()
    rows = native["aggregations"]["projects"]["buckets"]
    if missing == "project": rows.pop(0)
    elif missing == "summary": rows[0].pop("case_summary")
    elif missing == "ssm_bucket": rows[0]["case_summary"].pop("case_with_ssm")
    else: rows[0]["case_summary"]["case_with_ssm"].pop("doc_count")
    value = read(Fixture(native))
    assert value["projects"][0]["state"] == expected
    assert value["projects"][0]["cases_with_ssm_data"] is None
    assert value["projects"][0]["native_ssm_count_pointer"] is None
    assert value["coverage"]["observed_project_count"] == 1
    assert source.validate_gdc_ssm_coverage_bundle(value) == value


def test_successful_empty_terms_array_leaves_both_requested_projects_unknown():
    native = payload()
    native["aggregations"]["projects"]["buckets"] = []
    value = read(Fixture(native))
    assert value["coverage"]["returned_project_bucket_count"] == 0
    assert value["coverage"]["observed_project_count"] == 0
    assert all(row["cases_with_ssm_data"] is None for row in value["projects"])
    assert value["coverage"]["scientific_completeness"] == "unassessed"


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_approval_precedes_any_source_dispatch(approval):
    fixture = Fixture()
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        source.retrieve_gdc_ssm_coverage(approve_source_dispatch=approval, fetcher=fixture)
    assert not fixture.calls


@pytest.mark.parametrize("timeout", [True, 99, 30_001, None, "30000"])
def test_invalid_config_is_not_admitted(timeout):
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        source.GdcSsmCoverageRetrievalConfig(timeout)


def test_reflectively_mutated_config_is_revalidated_before_fetch():
    config, fixture = source.GdcSsmCoverageRetrievalConfig(), Fixture()
    object.__setattr__(config, "timeout_ms", True)
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        source.retrieve_gdc_ssm_coverage(config, approve_source_dispatch=True, fetcher=fixture)
    assert not fixture.calls


@pytest.mark.parametrize("clock", ["2026-02-30T00:00:00Z", "2026-10-27", None, 1])
def test_invalid_explicit_clock_refuses_before_fetch_except_automatic_none(clock):
    fixture = Fixture()
    if clock is None:
        assert source.retrieve_gdc_ssm_coverage(approve_source_dispatch=True, retrieved_at=clock, fetcher=fixture)["retrieved_at"].endswith("Z")
    else:
        with pytest.raises(source.GdcSsmCoverageRetrievalError):
            source.retrieve_gdc_ssm_coverage(approve_source_dispatch=True, retrieved_at=clock, fetcher=fixture)
        assert not fixture.calls


@pytest.mark.parametrize("body", [b'{}', b'[]', b'null', b'{"took":1,"took":2}', b'NaN', b'\xff', b'', b'{} trailing'])
def test_invalid_json_and_shape_never_become_empty_success(body):
    with pytest.raises((source.GdcSsmCoverageRetrievalError, ReviewedPublicHttpError)):
        read(Fixture(body=body))


@pytest.mark.parametrize("scope", ["root", "project", "summary", "ssm"])
def test_unreviewed_native_fields_refuse_including_case_identifiers(scope):
    native = payload()
    target = {"root": native, "project": native["aggregations"]["projects"]["buckets"][0],
        "summary": native["aggregations"]["projects"]["buckets"][0]["case_summary"],
        "ssm": native["aggregations"]["projects"]["buckets"][0]["case_summary"]["case_with_ssm"]}[scope]
    target["case_id"] = "unreviewed-individual-field"
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        read(Fixture(native))


@pytest.mark.parametrize("kind", ["timedout", "shards", "failed", "shards_type", "approx", "hits", "score", "error", "omitted", "boolean_error", "bucket_bound", "duplicate", "project_type", "negative", "boolean_count", "too_large", "ssm_exceeds_project", "ssm_exceeds_summary", "null_summary", "null_ssm", "inconsistent_hit_total"])
def test_native_incomplete_or_malformed_aggregates_refuse_whole_acquisition(kind):
    native = payload()
    aggregate = native["aggregations"]["projects"]
    row = aggregate["buckets"][0]
    if kind == "timedout": native["timed_out"] = True
    elif kind == "shards": native["_shards"]["successful"] = 11
    elif kind == "failed": native["_shards"]["failed"] = 1
    elif kind == "shards_type": native["_shards"]["total"] = True
    elif kind == "approx": native["hits"]["total"]["relation"] = "gte"
    elif kind == "hits": native["hits"]["hits"] = [{"case_id": "unadmitted"}]
    elif kind == "score": native["hits"]["max_score"] = 1
    elif kind == "error": aggregate["doc_count_error_upper_bound"] = 1
    elif kind == "omitted": aggregate["sum_other_doc_count"] = 1
    elif kind == "boolean_error": aggregate["doc_count_error_upper_bound"] = False
    elif kind == "bucket_bound": aggregate["buckets"] = [bucket(f"P{i}") for i in range(257)]
    elif kind == "duplicate": aggregate["buckets"].append(deepcopy(row))
    elif kind == "project_type": row["key"] = "bad/project"
    elif kind == "negative": row["doc_count"] = -1
    elif kind == "boolean_count": row["doc_count"] = True
    elif kind == "too_large": row["doc_count"] = 10**12+1
    elif kind == "ssm_exceeds_project": row["case_summary"]["case_with_ssm"]["doc_count"] = 21
    elif kind == "ssm_exceeds_summary": row["case_summary"]["doc_count"] = 1
    elif kind == "null_summary": row["case_summary"] = None
    elif kind == "null_ssm": row["case_summary"]["case_with_ssm"] = None
    else: native["hits"]["total"]["value"] = 33
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        read(Fixture(native))


def test_raw_response_bytes_and_native_projection_are_both_replayed():
    value = read()
    original = value["source_inputs"][0]
    assert hashlib.sha256(original["response_body"].encode()).hexdigest() == original["response_digest"]
    assert len(original["response_body"].encode()) == original["response_bytes"]
    original["native_payload"]["took"] = 4
    original["payload_digest"] = content_digest(original["native_payload"])
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        source.validate_gdc_ssm_coverage_bundle(restamp(value))


@pytest.mark.parametrize("change", ["count", "state", "pointer", "coverage", "response_digest", "bytes", "binding", "policy", "config", "receipt", "raw_bytes", "extra"])
def test_restamped_derived_or_transport_artifact_tampering_fails_native_replay(change):
    value = read()
    if change == "count": value["projects"][0]["cases_with_ssm_data"] = 1
    elif change == "state": value["projects"][0]["state"] = "project_bucket_missing"
    elif change == "pointer": value["projects"][0]["native_ssm_count_pointer"] = "/elsewhere"
    elif change == "coverage": value["coverage"]["gene_assay_eligibility"] = "established"
    elif change == "response_digest": value["source_inputs"][0]["response_digest"] = "a"*64
    elif change == "bytes": value["source_inputs"][0]["response_bytes"] += 1
    elif change == "binding": value["transport"]["binding_id"] = "bad"
    elif change == "policy": value["transport"]["transport_policy_digest"] = "a"*64
    elif change == "config": value["transport"]["config_digest"] = "a"*64
    elif change == "receipt": value["receipts"][0]["source_request_count"] = 0
    elif change == "raw_bytes": value["source_inputs"][0]["response_body"] += " "
    else: value["unreviewed"] = True
    with pytest.raises(source.GdcSsmCoverageRetrievalError):
        source.validate_gdc_ssm_coverage_bundle(restamp(value))


def test_callback_failure_is_sanitized_and_does_not_fall_back_or_retry():
    calls = []
    def fail(*_args, **_kwargs):
        calls.append(1)
        raise RuntimeError("private callback details")
    with pytest.raises(source.GdcSsmCoverageRetrievalError) as error:
        read(fail)
    assert "private callback details" not in str(error.value)
    assert len(calls) == 1


def test_callback_elapsed_deadline_is_checked_after_return():
    with patch.object(source, "monotonic", side_effect=[1.0, 32.0]):
        with pytest.raises(source.GdcSsmCoverageRetrievalError, match="elapsed_deadline"):
            read()


@pytest.mark.parametrize("field,value", [("status", 302), ("request_url", "https://elsewhere.example/"),
    ("config_digest", "a"*64), ("transport_policy_digest", "a"*64), ("body_sha256", "a"*64),
    ("response_bytes", 1), ("content_type", "text/plain")])
def test_reflectively_mutated_typed_response_is_readmitted(field, value):
    fixture = Fixture()
    def changed(url, *, http_config):
        response = fixture(url, http_config=http_config)
        object.__setattr__(response, field, value)
        return response
    with pytest.raises((source.GdcSsmCoverageRetrievalError, ReviewedPublicHttpError)):
        read(changed)


def test_registration_is_inert_default_catalogue_stable_and_fixture_identity_nonce_bound():
    fixture = Fixture()
    before = builtin_workflow_operation_catalog().to_dict()
    first = source.register_gdc_ssm_coverage_operations(WorkflowOperationCatalog(), fetcher=fixture)
    second = source.register_gdc_ssm_coverage_operations(WorkflowOperationCatalog(), fetcher=fixture)
    assert not fixture.calls
    assert first.catalog_digest != second.catalog_digest
    assert builtin_workflow_operation_catalog().to_dict() == before
    assert first.operations()[0].integration_id == "gdc"


def test_returned_artifacts_are_detached_from_caller_native_objects():
    native = payload()
    value = read(Fixture(native))
    native["aggregations"]["projects"]["buckets"][0]["doc_count"] = 100
    assert source.validate_gdc_ssm_coverage_bundle(value) == value
