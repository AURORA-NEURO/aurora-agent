"""Claims for the fixed, public TP53 annotation reader; fixture observations are not live services."""

from __future__ import annotations

import json
import socket
import threading
from unittest.mock import patch

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.reviewed_ensembl_gene_retrieval import (
    ENSEMBL_GENE_OPERATION_ID, MAX_ENSEMBL_RESPONSE_BYTES,
    EnsemblGeneHttpResponse, EnsemblGeneRetrievalConfig, EnsemblGeneRetrievalError,
    _DeadlineReader, _default_fetch, _resolve_public_address,
    register_ensembl_gene_operations, retrieve_ensembl_gene_references,
    validate_ensembl_gene_bundle,
)
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperationCatalog


CLOCK = "2026-10-27T00:00:00Z"
LOOKUP = "https://rest.ensembl.org/lookup/symbol/homo_sapiens/TP53?expand=0"
GENE_ID = "ENSG00000141510"
XREF = (f"https://rest.ensembl.org/xrefs/id/{GENE_ID}"
        "?external_db=EntrezGene&object_type=gene&species=homo_sapiens&all_levels=0")


def _gene():
    return {"id": GENE_ID, "display_name": "TP53", "species": "homo_sapiens", "object_type": "Gene",
            "db_type": "core", "version": 20, "assembly_name": "GRCh38", "biotype": "protein_coding",
            "source": "ensembl_havana", "description": "tumor protein p53", "canonical_transcript": "not-retained",
            "start": 7661779, "end": 7687546}


def _reference(identifier="7157"):
    return {"primary_id": identifier, "display_id": "TP53", "dbname": "EntrezGene", "version": "0",
            "info_type": "DEPENDENT", "description": None, "synonyms": ["not-retained"]}


class Fixture:
    def __init__(self, gene=None, references=None):
        self.gene = _gene() if gene is None else gene
        self.references = [_reference()] if references is None else references
        self.calls = []

    def __call__(self, url, *, timeout_seconds, max_response_bytes):
        self.calls.append({"url": url, "timeout": timeout_seconds, "maximum": max_response_bytes})
        value = self.gene if len(self.calls) == 1 else self.references
        return EnsemblGeneHttpResponse(200, json.dumps(value).encode(), url, {"Content-Type": "application/json"})


def _read(fixture=None, **kwargs):
    return retrieve_ensembl_gene_references(approve_source_dispatch=True, retrieved_at=CLOCK,
                                            fetcher=fixture or Fixture(), **kwargs)


def _reseal(bundle):
    bundle["bundle_digest"] = content_digest({key: value for key, value in bundle.items() if key != "bundle_digest"})


def test_two_exact_serial_routes_retain_source_candidates_and_replay_without_fetching():
    fixture = Fixture(references=[_reference("7157"), _reference("12345")])
    bundle = _read(fixture)
    assert [call["url"] for call in fixture.calls] == [LOOKUP, XREF]
    assert all(0 < call["timeout"] <= 30 and call["maximum"] == 1_000_000 for call in fixture.calls)
    assert fixture.calls[1]["timeout"] <= fixture.calls[0]["timeout"]
    assert bundle["gene"]["ensembl_gene_id"] == GENE_ID
    assert [row["entrez_gene_id"] for row in bundle["entrez_candidates"]] == ["7157", "12345"]
    assert bundle["coverage"]["reported_total_count"] is None
    assert bundle["coverage"]["crosswalk_validation"] == "source_assertions_only"
    assert bundle["coverage"]["exhaustiveness"] == "unassessed"
    assert validate_ensembl_gene_bundle(bundle) == bundle
    assert len(fixture.calls) == 2
    assert "canonical_transcript" not in json.dumps(bundle)
    assert "not-retained" not in json.dumps(bundle)
    assert "live_verified" not in json.dumps(bundle)
    assert bundle["transport"]["mode"] == "caller_supplied"


def test_successful_empty_reference_array_is_explicit_empty_success_and_not_missing_gene():
    bundle = _read(Fixture(references=[]))
    assert bundle["entrez_candidates"] == []
    assert bundle["coverage"]["returned_gene_count"] == 1
    assert bundle["coverage"]["returned_candidate_count"] == 0
    assert bundle["coverage"]["candidate_state"] == "empty_success"
    assert bundle["coverage"]["reported_total_count"] is None
    validate_ensembl_gene_bundle(bundle)


@pytest.mark.parametrize("approval", [False, 0, 1, "yes", None])
def test_only_literal_source_approval_can_reach_either_request(approval):
    fixture = Fixture()
    with pytest.raises(EnsemblGeneRetrievalError, match="approval"):
        retrieve_ensembl_gene_references(approve_source_dispatch=approval, fetcher=fixture)
    assert fixture.calls == []


@pytest.mark.parametrize("changes", [{"gene_symbol": "IDH1"}, {"gene_symbol": "tp53"},
    {"timeout_ms": 99}, {"timeout_ms": 30001}, {"timeout_ms": True}, {"timeout_ms": 30000.0}])
def test_config_rejects_unreviewed_symbols_and_invalid_total_budgets(changes):
    with pytest.raises(EnsemblGeneRetrievalError):
        EnsemblGeneRetrievalConfig(**changes)


@pytest.mark.parametrize("changes", [{"species": "mus_musculus"}, {"object_type": "Transcript"},
    {"db_type": "otherfeatures"}, {"display_name": "P53"}, {"id": "ENST00000141510"},
    {"id": "ENSG00000141510.20"}, {"id": "ENSG00000141510/other"}, {"version": True}, {"version": 0}])
def test_lookup_scope_or_identifier_mismatch_never_initiates_a_reference_request(changes):
    gene = _gene()
    gene.update(changes)
    fixture = Fixture(gene=gene)
    with pytest.raises(EnsemblGeneRetrievalError):
        _read(fixture)
    assert len(fixture.calls) == 1


@pytest.mark.parametrize("references", [[_reference()] * 2, [_reference(str(index + 1)) for index in range(65)],
    [{**_reference(), "dbname": "HGNC"}], [{**_reference(), "primary_id": 7157}],
    [{**_reference(), "primary_id": "0"}], [{**_reference(), "display_id": None}], {}, None])
def test_reference_ambiguity_scope_and_shape_fail_instead_of_truncation_or_substitution(references):
    fixture = Fixture()
    fixture.references = references
    with pytest.raises(EnsemblGeneRetrievalError):
        _read(fixture)
    assert len(fixture.calls) == 2


def test_exactly_64_distinct_reference_candidates_are_retained_with_no_join():
    bundle = _read(Fixture(references=[_reference(str(index + 1)) for index in range(64)]))
    assert len(bundle["entrez_candidates"]) == 64
    assert bundle["coverage"]["crosswalk_validation"] == "source_assertions_only"
    assert validate_ensembl_gene_bundle(bundle) == bundle


@pytest.mark.parametrize("status,code", [(400, "source_lookup_refused"), (403, "source_forbidden"),
    (404, "source_route_not_found"), (408, "source_timeout"), (429, "source_rate_limited"),
    (503, "source_unavailable"), (500, "source_http_failure")])
def test_http_refusals_never_become_empty_success_and_do_not_expose_payloads(status, code):
    calls = []
    def fetch(url, **_kwargs):
        calls.append(url)
        return EnsemblGeneHttpResponse(status, b'{"error":"private upstream details"}', url,
                                      {"Retry-After": "1.25", "X-Private": "do not retain"})
    with pytest.raises(EnsemblGeneRetrievalError) as caught:
        _read(fetch)
    error = caught.value
    assert error.failure_class == code and error.status_code == status
    assert error.retry_after_seconds == (1.25 if status == 429 else None)
    assert len(calls) == 1
    assert "private" not in str(error)
    assert "private" not in json.dumps(error.to_dict())
    assert error.to_dict()["automatic_retry"] is False


@pytest.mark.parametrize("retry", ["NaN", "inf", "-1", "999999", "1.0\r\nSecret: value", "Wed, 28 Oct 2026 00:00:00 GMT", "a" * 40])
def test_unbounded_or_ambiguous_retry_after_is_unknown_without_sleep_or_retry(retry):
    def fetch(url, **_kwargs):
        return EnsemblGeneHttpResponse(429, b"", url, {"Retry-After": retry})
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.sleep") as sleep:
        with pytest.raises(EnsemblGeneRetrievalError) as caught:
            _read(fetch)
    assert caught.value.retry_after_seconds is None
    sleep.assert_not_called()


@pytest.mark.parametrize("response", [
    EnsemblGeneHttpResponse(302, b"", LOOKUP, {}),
    EnsemblGeneHttpResponse(200, b"{}", LOOKUP + "&redirect=1", {"Content-Type": "application/json"}),
    EnsemblGeneHttpResponse(200, b"{}", LOOKUP, {"Content-Type": "text/html"}),
    EnsemblGeneHttpResponse(200, b"{}", LOOKUP, {"Content-Type": "application/json", "content-type": "application/json"}),
    EnsemblGeneHttpResponse(True, b"{}", LOOKUP, {"Content-Type": "application/json"}),
    EnsemblGeneHttpResponse(200, b"x" * (MAX_ENSEMBL_RESPONSE_BYTES + 1), LOOKUP, {"Content-Type": "application/json"}),
    EnsemblGeneHttpResponse(200, b'{"id":"a","id":"b"}', LOOKUP, {"Content-Type": "application/json"}),
    EnsemblGeneHttpResponse(200, b'{"extra":NaN}', LOOKUP, {"Content-Type": "application/json"}),
    EnsemblGeneHttpResponse(200, b"[]", LOOKUP, {"Content-Type": "application/json"}),
])
def test_redirect_media_size_duplicate_and_nonfinite_responses_are_refused(response):
    calls = []
    def fetch(url, **_kwargs):
        calls.append(url)
        return response
    with pytest.raises(EnsemblGeneRetrievalError):
        _read(fetch)
    assert calls == [LOOKUP]


def test_tree_depth_and_node_limits_apply_before_whitelisting_unknown_fields():
    for extra in ([0] * 50001, [[[[0]]]]):
        gene = _gene()
        gene["extra"] = extra
        if len(extra) < 100:
            for _ in range(33):
                gene = {"extra": gene}
        with pytest.raises(EnsemblGeneRetrievalError, match="tree_bound"):
            _read(Fixture(gene=gene))


def test_caller_transport_exception_details_are_withheld():
    def fetch(_url, **_kwargs):
        raise RuntimeError("secret callback diagnostic")
    with pytest.raises(EnsemblGeneRetrievalError) as caught:
        _read(fetch)
    assert caught.value.failure_class == "application_transport_failure"
    assert caught.value.__cause__ is None
    assert "secret" not in str(caught.value)


def test_late_injected_response_is_refused_and_cannot_trigger_second_dispatch():
    elapsed = [100.0]
    fixture = Fixture()
    def fetch(*args, **kwargs):
        result = fixture(*args, **kwargs)
        elapsed[0] += 31
        return result
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.monotonic", side_effect=lambda: elapsed[0]):
        with pytest.raises(EnsemblGeneRetrievalError, match="deadline"):
            _read(fetch)
    assert len(fixture.calls) == 1


@pytest.mark.parametrize("clock", ["", "2026-10-27T00:00:00.1Z", "2026-02-30T00:00:00Z", "2026-10-27T00:00:00+00:00"])
def test_clock_validation_precedes_source_calls(clock):
    fixture = Fixture()
    with pytest.raises(EnsemblGeneRetrievalError):
        retrieve_ensembl_gene_references(approve_source_dispatch=True, retrieved_at=clock, fetcher=fixture)
    assert fixture.calls == []


@pytest.mark.parametrize("edit", [
    lambda value: value["gene"].update(ensembl_gene_id="ENSG00000000001"),
    lambda value: value["coverage"].update(reported_total_count=1),
    lambda value: value["coverage"].update(crosswalk_validation="verified"),
    lambda value: value["source_inputs"][1].update(url=LOOKUP),
    lambda value: value["source_inputs"][0]["native_payload"].update(display_name="IDH1"),
    lambda value: value["source_inputs"][0].update(payload_digest="0" * 64),
    lambda value: value["receipts"][0].update(request_digest="0" * 64),
    lambda value: value["transport"].update(timing_authority="adapter_total_deadline"),
    lambda value: value.update(unreviewed_extra="not admitted"),
])
def test_bundle_validator_replays_source_projections_and_rejects_restamped_derived_tampering(edit):
    bundle = _read()
    edit(bundle)
    _reseal(bundle)
    with pytest.raises(EnsemblGeneRetrievalError):
        validate_ensembl_gene_bundle(bundle)


def test_validator_returns_a_defensive_copy_and_retains_nullable_annotation_fields():
    bundle = _read()
    checked = validate_ensembl_gene_bundle(bundle)
    checked["gene"]["description"] = "changed"
    assert bundle["gene"]["description"] == "tumor protein p53"
    assert bundle["entrez_candidates"][0]["description"] is None


def test_registration_is_inert_default_bindings_are_stable_and_injected_sessions_are_distinct():
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval._default_fetch") as default:
        first = register_ensembl_gene_operations(WorkflowOperationCatalog())
        second = register_ensembl_gene_operations(WorkflowOperationCatalog())
    default.assert_not_called()
    assert first.catalog_digest == second.catalog_digest
    fixture = Fixture()
    injected = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    other = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    assert fixture.calls == []
    assert len({first.catalog_digest, injected.catalog_digest, other.catalog_digest}) == 3
    assert len(injected.operations()) == 1
    assert injected.resolve(ENSEMBL_GENE_OPERATION_ID).integration_id == "ensembl"


def test_catalogue_rejects_unreviewed_arguments_and_requires_source_approval():
    fixture = Fixture()
    catalog = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    for arguments, approval in [({"symbol": "IDH1"}, True), ({"timeout_ms": 99}, True), ({}, False)]:
        with pytest.raises(ValueError):
            catalog.execute(ENSEMBL_GENE_OPERATION_ID, arguments, approve_source_dispatch=approval)
    assert fixture.calls == []
    result = catalog.execute(ENSEMBL_GENE_OPERATION_ID, {}, approve_source_dispatch=True, retrieved_at=CLOCK)
    assert validate_ensembl_gene_bundle(result.value) == result.value
    assert result.receipt["transport_mode"] == "caller_supplied"
    assert result.receipt["candidate_count"] == 1
    assert "source_inputs" not in result.receipt


def test_checkpoint_resume_does_not_repeat_completed_annotation_calls():
    fixture = Fixture()
    catalog = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    proposal = {"name": "TP53 references", "steps": [{"id": "gene", "operation": ENSEMBL_GENE_OPERATION_ID,
                 "arguments": {}, "depends_on": []}], "outputs": ["gene"]}
    plan = compile_workflow_plan(proposal, catalog, task="Read TP53 references")
    first = execute_workflow_plan(plan, catalog, approved=True, retrieved_at=CLOCK)
    assert first.status == "completed"
    assert len(fixture.calls) == 2
    resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=first.checkpoint, retrieved_at=CLOCK)
    assert resumed.status == "completed" and len(fixture.calls) == 2
    replaced_catalog = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=Fixture())
    with pytest.raises(ValueError):
        execute_workflow_plan(plan, replaced_catalog, approved=True, checkpoint=first.checkpoint, retrieved_at=CLOCK)


class FakeSocket:
    def __init__(self, blocks):
        self.blocks = list(blocks)
        self.timeouts = []
        self.closed = False
        self.sent = []

    def settimeout(self, timeout):
        self.timeouts.append(timeout)

    def recv(self, _size):
        return self.blocks.pop(0) if self.blocks else b""

    def connect(self, address):
        self.address = address

    def sendall(self, value):
        self.sent.append(value)

    def close(self):
        self.closed = True


def test_default_transport_pins_public_dns_tls_host_exact_get_and_bounded_response():
    body = json.dumps(_gene()).encode()
    raw = FakeSocket([])
    tls = FakeSocket([b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                      + str(len(body)).encode() + b"\r\n\r\n" + body])
    address = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.socket", return_value=raw), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.ssl.create_default_context") as context:
        context.return_value.wrap_socket.return_value = tls
        response = _default_fetch(LOOKUP, timeout_seconds=1, max_response_bytes=1_000_000)
    assert response.body == body and response.final_url == LOOKUP
    assert raw.address == address[4]
    context.return_value.wrap_socket.assert_called_once_with(raw, server_hostname="rest.ensembl.org")
    assert tls.sent[0].startswith(b"GET /lookup/symbol/homo_sapiens/TP53?expand=0 HTTP/1.1\r\n")
    assert b"Authorization" not in tls.sent[0]
    assert tls.closed and all(0 < value <= 1 for value in tls.timeouts)


def test_chunked_default_response_is_decoded_with_the_same_response_bound():
    body = b'{"id":"ENSG00000141510"}'
    raw = FakeSocket([])
    wire = (b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
            + format(len(body), "x").encode() + b"\r\n" + body + b"\r\n0\r\n\r\n")
    tls = FakeSocket([wire])
    address = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.socket", return_value=raw), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.ssl.create_default_context") as context:
        context.return_value.wrap_socket.return_value = tls
        response = _default_fetch(LOOKUP, timeout_seconds=1, max_response_bytes=1_000_000)
    assert response.body == body and tls.closed


@pytest.mark.parametrize("headers", [b"Content-Type: application/json\r\nContent-Type: application/json\r\n",
    b"Content-Type: application/json\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n"])
def test_default_transport_rejects_duplicate_headers_and_ambiguous_framing(headers):
    raw = FakeSocket([])
    tls = FakeSocket([b"HTTP/1.1 200 OK\r\n" + headers + b"\r\n{}"])
    address = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo", return_value=[address]), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.socket", return_value=raw), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.ssl.create_default_context") as context:
        context.return_value.wrap_socket.return_value = tls
        with pytest.raises(EnsemblGeneRetrievalError, match="ambiguous_response"):
            _default_fetch(LOOKUP, timeout_seconds=1, max_response_bytes=1_000_000)
    assert tls.closed


def test_slow_header_and_body_trickles_cannot_extend_remaining_deadline():
    reader = _DeadlineReader(FakeSocket([b"a", b"b", b"\n"]), 10.0)
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.monotonic", side_effect=[0, 1, 2, 3, 4, 11]):
        with pytest.raises(EnsemblGeneRetrievalError, match="deadline"):
            reader.readline(100)
    reader = _DeadlineReader(FakeSocket([b"abc", b"def"]), 10.0)
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.monotonic", side_effect=[0, 1, 2, 3, 11]):
        with pytest.raises(EnsemblGeneRetrievalError, match="deadline"):
            reader.read(6)


@pytest.mark.parametrize("address", ["127.0.0.1", "10.0.0.1", "::1", "169.254.169.254", "224.0.0.1"])
def test_nonpublic_dns_is_refused_before_socket_creation(address):
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo", return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, "", (address, 443))]), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.socket") as connection:
        with pytest.raises(EnsemblGeneRetrievalError, match="not_public"):
            _default_fetch(LOOKUP, timeout_seconds=1, max_response_bytes=1_000_000)
    connection.assert_not_called()


def test_dns_wait_is_bounded_and_late_result_never_initiates_http():
    released = threading.Event()
    entered = threading.Event()
    def resolve(*_args, **_kwargs):
        entered.set()
        released.wait(2)
        return [(socket.AF_INET, socket.SOCK_STREAM, 6, "", ("93.184.216.34", 443))]
    try:
        with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo", side_effect=resolve), \
             patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.socket") as connection:
            with pytest.raises(EnsemblGeneRetrievalError, match="deadline"):
                _default_fetch(LOOKUP, timeout_seconds=0.02, max_response_bytes=1_000_000)
            assert entered.is_set()
            connection.assert_not_called()
    finally:
        released.set()


def test_exhausted_dns_worker_slots_are_bounded_before_new_thread_creation():
    semaphore = threading.BoundedSemaphore(1)
    semaphore.acquire()
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval._DNS_SLOTS", semaphore), \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.threading.Thread") as thread, \
         patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.monotonic", return_value=0):
        with pytest.raises(EnsemblGeneRetrievalError, match="capacity_or_deadline"):
            _resolve_public_address(0.01)
    thread.assert_not_called()
    semaphore.release()


@pytest.mark.parametrize("url", ["http://rest.ensembl.org/lookup/symbol/homo_sapiens/TP53?expand=0",
    LOOKUP + "&expand=1", LOOKUP.replace("rest.ensembl.org", "localhost"),
    XREF.replace(GENE_ID, "ENSG00000141510/../../other"), XREF.replace("EntrezGene", "HGNC")])
def test_default_transport_refuses_unreviewed_routes_before_dns(url):
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.socket.getaddrinfo") as dns:
        with pytest.raises(EnsemblGeneRetrievalError, match="route_is_not_reviewed"):
            _default_fetch(url, timeout_seconds=1, max_response_bytes=1_000_000)
    dns.assert_not_called()


def test_header_bytes_have_a_separate_bound_before_body_parsing():
    reader = _DeadlineReader(FakeSocket([b"x" * 33000 + b"\n", b"y" * 33000 + b"\n"]), 10.0)
    with patch("prism_sdk.reviewed_ensembl_gene_retrieval.time.monotonic", return_value=0):
        reader.readline(65537)
        with pytest.raises(EnsemblGeneRetrievalError, match="header_bound"):
            reader.readline(65537)


def test_native_output_schema_rejects_candidate_type_changes_before_catalogue_return():
    fixture = Fixture()
    catalog = register_ensembl_gene_operations(WorkflowOperationCatalog(), fetcher=fixture)
    schema = catalog.resolve(ENSEMBL_GENE_OPERATION_ID).output_schema
    bundle = _read()
    bundle["entrez_candidates"][0]["entrez_gene_id"] = 7157
    with pytest.raises(ValueError):
        catalog._validate(ENSEMBL_GENE_OPERATION_ID, bundle, schema)
