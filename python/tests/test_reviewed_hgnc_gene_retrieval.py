"""Offline native identity/status/missingness and exact HGNC byte replay invariants."""
from copy import deepcopy
import hashlib
import json
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk import reviewed_hgnc_gene_retrieval as source
from prism_sdk.reviewed_public_http import ReviewedPublicHttpError
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, builtin_workflow_operation_catalog

CLOCK = "2026-10-27T00:00:00Z"


def gene():
    return {"hgnc_id": "HGNC:11998", "symbol": "TP53", "name": "tumor protein p53", "status": "Approved",
        "alias_symbol": ["P53", "BCC7", "P53"], "alias_name": ["Fixture alias name"],
        "prev_symbol": [], "prev_name": None, "entrez_id": "7157", "ensembl_gene_id": "ENSG00000141510",
        "gene_group": ["Fixture group"], "_version_": 9223372036854775809}


def payload(docs=None):
    docs = [gene()] if docs is None else docs
    return {"responseHeader": {"status": 0, "QTime": 1}, "response": {"numFound": len(docs), "start": 0, "docs": docs}}


class Fixture:
    def __init__(self, value=None, body=None): self.value, self.body, self.calls = payload() if value is None else value, body, []
    def __call__(self, url, *, http_config):
        self.calls.append(url)
        body = json.dumps(self.value).encode() if self.body is None else self.body
        return source.HgncGeneHttpResponse(200, body, url, len(body), hashlib.sha256(body).hexdigest(),
            http_config.transport_policy_digest, http_config.config_digest, "application/json")


def read(fixture=None):
    return source.retrieve_hgnc_gene_references(approve_source_dispatch=True, retrieved_at=CLOCK,
        fetcher=Fixture() if fixture is None else fixture)


def test_fixed_one_get_retains_every_alias_status_identifier_and_large_native_integer():
    fixture = Fixture()
    value = read(fixture)
    assert fixture.calls == ["https://rest.genenames.org/fetch/symbol/TP53"]
    assert value["records"][0]["native_record"] == gene()
    assert value["records"][0]["field_observations"]["alias_symbol"]["value"] == ["P53", "BCC7", "P53"]
    assert value["records"][0]["peripheral_native_fields"]["_version_"] == 9223372036854775809
    assert value["coverage"]["selection_state"] == "single_approved_source_record"
    assert value["coverage"]["scientific_identifier_equivalence"] == "unestablished"
    assert source.validate_hgnc_gene_reference_bundle(value) == value


@pytest.mark.parametrize("kind,expected", [("absent", "missing"), ("null", "null"), ("empty", "empty"), ("value", "observed")])
def test_missing_null_empty_and_observed_alias_fields_are_distinct(kind, expected):
    row = gene()
    if kind == "absent": row.pop("alias_symbol")
    else: row["alias_symbol"] = {"null": None, "empty": [], "value": ["P53"]}[kind]
    value = read(Fixture(payload([row])))
    observation = value["records"][0]["field_observations"]["alias_symbol"]
    assert observation["state"] == expected
    assert observation["native_pointer"] is None if kind == "absent" else observation["native_pointer"].endswith("/alias_symbol")


@pytest.mark.parametrize("kind,state", [("none", "not_found"), ("withdrawn", "withdrawn"),
    ("multiple", "ambiguous_multiple_records"), ("missing_status", "missing_status"),
    ("null_identity", "missing_identity"), ("other_symbol", "selector_value_mismatch"),
    ("other_status", "unreviewed_status"), ("partial", "source_window_incomplete"),
    ("approximate", "source_total_approximate")])
def test_nonready_source_outcomes_are_retained_without_selecting_an_identity(kind, state):
    native = payload()
    row = native["response"]["docs"][0]
    if kind == "none": native = payload([])
    elif kind == "withdrawn": row["status"] = "Entry Withdrawn"
    elif kind == "multiple": native = payload([gene(), {**gene(), "hgnc_id": "HGNC:999"}])
    elif kind == "missing_status": row.pop("status")
    elif kind == "null_identity": row["hgnc_id"] = None
    elif kind == "other_symbol": row["symbol"] = "tp53"
    elif kind == "other_status": row["status"] = "Unreviewed status"
    elif kind == "partial": native["response"]["numFound"] = 3
    else: native["response"]["numFoundExact"] = False
    value = read(Fixture(native))
    assert value["coverage"]["selection_state"] == state
    assert value["coverage"]["source_identity_ready"] is False
    assert value["coverage"]["records_discarded"] == 0
    assert value["source_inputs"][0]["native_payload"] == native
    assert source.validate_hgnc_gene_reference_bundle(value) == value


def test_partial_source_window_retains_unknown_omissions_and_all_returned_candidates():
    native = payload([gene(), {**gene(), "hgnc_id": "HGNC:999"}])
    native["response"]["numFound"] = 5
    value = read(Fixture(native))
    assert len(value["records"]) == 2 and value["coverage"]["unreturned_reported_count"] == 3
    assert value["coverage"]["unreturned_influence"] == "unknown"


@pytest.mark.parametrize("field,value", [("entrez_id", 7157), ("ensembl_gene_id", ["ENSG00000141510"]),
    ("hgnc_id", "11998"), ("entrez_id", "07157"), ("ensembl_gene_id", "ENSMUSG00000141510"),
    ("alias_symbol", "P53"), ("alias_name", [1]), ("sequence", "ACGT"), ("patient_id", "unadmitted"),
    ("_version_", {}), ("name", "bad\x00text"), ("symbol", True)])
def test_wrong_types_unreviewed_fields_and_identifier_coercions_refuse(field, value):
    row = gene(); row[field] = value
    with pytest.raises(source.HgncGeneRetrievalError): read(Fixture(payload([row])))


@pytest.mark.parametrize("kind", ["status", "negative_total", "bool_total", "bad_start", "too_many_docs", "wrong_docs", "unknown_envelope", "unknown_response", "false_exact_type", "window", "deep", "long_aliases"])
def test_malformed_or_unbounded_source_envelopes_are_not_empty_success(kind):
    value = payload()
    if kind == "status": value["responseHeader"]["status"] = 1
    elif kind == "negative_total": value["response"]["numFound"] = -1
    elif kind == "bool_total": value["response"]["numFound"] = True
    elif kind == "bad_start": value["response"]["start"] = "0"
    elif kind == "too_many_docs": value = payload([gene() for _ in range(9)])
    elif kind == "wrong_docs": value["response"]["docs"] = {}
    elif kind == "unknown_envelope": value["unexpected"] = True
    elif kind == "unknown_response": value["response"]["next"] = "https://example.invalid/"
    elif kind == "false_exact_type": value["response"]["numFoundExact"] = 0
    elif kind == "window": value["response"]["start"] = 1
    elif kind == "deep":
        nested = {}; cursor = nested
        for _ in range(24): cursor["child"] = {}; cursor = cursor["child"]
        value["responseHeader"]["params"] = nested
    else: value["response"]["docs"][0]["alias_symbol"] = ["P53"]*257
    with pytest.raises(source.HgncGeneRetrievalError): read(Fixture(value))


@pytest.mark.parametrize("body", [b'{}', b'[]', b'null', b'{"a":1,"a":2}', b'NaN', b'\xff', b'', b'{} trailing'])
def test_nonfinite_duplicate_nonobject_and_malformed_json_refuse(body):
    with pytest.raises((source.HgncGeneRetrievalError, ReviewedPublicHttpError)): read(Fixture(body=body))


@pytest.mark.parametrize("approval", [False, None, 1, "true"])
def test_literal_approval_precedes_any_callback(approval):
    fixture = Fixture()
    with pytest.raises(source.HgncGeneRetrievalError): source.retrieve_hgnc_gene_references(approve_source_dispatch=approval, fetcher=fixture)
    assert not fixture.calls


@pytest.mark.parametrize("timeout", [True, 0, 99, 30001, "30000"])
def test_invalid_timeout_fails_before_transport(timeout):
    with pytest.raises(source.HgncGeneRetrievalError): source.HgncGeneRetrievalConfig(timeout_ms=timeout)


@pytest.mark.parametrize("change", ["native", "state", "count", "raw", "digest", "pointer", "config", "receipt", "clinical"])
def test_restamped_artifact_changes_fail_raw_byte_replay(change):
    value = read()
    if change == "native": value["source_inputs"][0]["native_payload"]["response"]["docs"][0]["symbol"] = "different"
    elif change == "state": value["coverage"]["selection_state"] = "approved"
    elif change == "count": value["coverage"]["returned_record_count"] = 0
    elif change == "raw": value["source_inputs"][0]["response_body"] += " "
    elif change == "digest": value["source_inputs"][0]["response_digest"] = "a"*64
    elif change == "pointer": value["records"][0]["native_pointer"] = "/elsewhere"
    elif change == "config": value["transport"]["config_digest"] = "a"*64
    elif change == "receipt": value["receipts"][0]["source_request_count"] = 0
    else: value["clinical_actions_authorized"] = True
    value["bundle_digest"] = content_digest({key:item for key,item in value.items() if key != "bundle_digest"})
    with pytest.raises(source.HgncGeneRetrievalError): source.validate_hgnc_gene_reference_bundle(value)


def test_injected_failure_is_sanitized_without_retry_or_empty_result():
    calls = []
    def fail(*_args, **_kwargs): calls.append(1); raise RuntimeError("private foreign details")
    with pytest.raises(source.HgncGeneRetrievalError) as error: read(fail)
    assert calls == [1] and "private foreign details" not in str(error.value)


def test_callback_elapsed_deadline_is_checked():
    with patch.object(source, "monotonic", side_effect=[0., 31.]):
        with pytest.raises(source.HgncGeneRetrievalError, match="deadline"): read()


def test_rate_admission_refuses_early_start_without_sleep_or_request():
    with patch.object(source, "_NEXT_ADMISSION", 10.1), patch.object(source, "monotonic", return_value=10.):
        with pytest.raises(source.HgncGeneRetrievalError, match="rate_admission"): source._admit_start()


def test_registration_is_inert_nonce_binds_fixture_and_leaves_default_catalogue_unchanged():
    before = builtin_workflow_operation_catalog().to_dict(); fixture = Fixture()
    one = source.register_hgnc_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    two = source.register_hgnc_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    assert one.catalog_digest != two.catalog_digest and not fixture.calls
    assert builtin_workflow_operation_catalog().to_dict() == before


@pytest.mark.parametrize("field,value", [("body", b'{}'), ("config_digest", "a"*64), ("request_url", "https://example.invalid/"), ("status", 302)])
def test_mutated_typed_response_is_readmitted(field, value):
    fixture = Fixture()
    def changed(url, *, http_config):
        response = fixture(url, http_config=http_config); object.__setattr__(response, field, value); return response
    with pytest.raises((source.HgncGeneRetrievalError, ReviewedPublicHttpError)): read(changed)


@pytest.mark.parametrize("tag,state", [("Ambiguous", "ambiguous_source_tag"),
    ("Ambigous", "ambiguous_source_tag"), ("Placeholder symbol", "placeholder_source_tag"),
    ("Future source tag", "unreviewed_source_tag"), ("ambiguous", "unreviewed_source_tag")])
def test_explicit_nonready_symbol_tag_cannot_promote_approved_record_to_ready_identity(tag, state):
    value = read(Fixture(payload([{**gene(), "symbol_report_tag": tag}])))
    assert value["records"][0]["state"] == value["coverage"]["selection_state"] == state
    assert value["coverage"]["source_identity_ready"] is False
    assert value["records"][0]["field_observations"]["symbol_report_tag"]["value"] == tag
    assert source.validate_hgnc_gene_reference_bundle(value) == value


@pytest.mark.parametrize("tag", [None, "", "Stable symbol"])
def test_missing_information_is_not_invented_from_null_empty_or_stable_source_tag(tag):
    value = read(Fixture(payload([{**gene(), "symbol_report_tag": tag}])))
    assert value["coverage"]["selection_state"] == "single_approved_source_record"
    assert value["records"][0]["field_observations"]["symbol_report_tag"]["value"] == tag
    assert value["coverage"]["scientific_identifier_equivalence"] == "unestablished"
