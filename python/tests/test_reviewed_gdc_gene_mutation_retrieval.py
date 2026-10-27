"""Focused invariants for one reviewed aggregate GDC query; all tests stay offline."""
from copy import deepcopy
from dataclasses import replace
import json
import socket
import threading
import time
import unittest
from unittest.mock import Mock, patch
from urllib.parse import parse_qs, urlsplit

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk import reviewed_gdc_gene_mutation_retrieval as gdc

_AT = "2026-10-26T12:00:00Z"
_TRANSPORT = {"transport_id": "fixture.gdc-gene-mutation", "transport_config_digest": "a" * 64}


def _bucket(project, count=10, *, missing_gene=False):
    return {"key": project, "doc_count": count, "genes": {"doc_count": count * 20, "my_genes": {"doc_count": 0 if missing_gene else count, "gene_id": {"doc_count_error_upper_bound": 0, "sum_other_doc_count": 0, "buckets": [] if missing_gene else [{"key": gdc.REVIEWED_GDC_GENE_MUTATION_GENE_ID, "doc_count": count}]}}}}


def _response():
    return {"took": 3, "timed_out": False, "_shards": {"total": 12, "successful": 12, "skipped": 0, "failed": 0}, "hits": {"total": {"value": 30, "relation": "eq"}, "max_score": None, "hits": []}, "aggregations": {"projects": {"doc_count_error_upper_bound": 0, "sum_other_doc_count": 0, "buckets": [_bucket("TCGA-GBM", 10), _bucket("TCGA-LGG", 0), _bucket("TCGA-BRCA", 20)]}}}


def _adapter(response=None):
    calls = []
    def fetch(url):
        calls.append(url)
        return deepcopy(_response() if response is None else response)
    adapter = gdc.ReviewedGdcGeneMutationRetrievalAdapter(gdc.ReviewedGdcGeneMutationRetrievalConfig(**_TRANSPORT), fetch=fetch)
    return adapter, calls


def _execute(response=None):
    adapter, _ = _adapter(response)
    return adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_AT)


class _Clock:
    def __init__(self):
        self.now = 0.0
    def monotonic(self):
        return self.now


class _WireSocket:
    """Socket bytes go through the real HTTPResponse parser, with optional elapsed recv time."""
    def __init__(self, fragments, clock=None):
        self.fragments = [(item, 0.0) if isinstance(item, bytes) else item for item in fragments]
        self.clock, self.closed, self.timeouts, self.recv_calls = clock, False, [], 0
    def settimeout(self, value):
        if not 0 < value <= 30:
            raise AssertionError("socket deadline was not bounded")
        self.timeouts.append(value)
    def recv(self, maximum):
        self.recv_calls += 1
        if not self.fragments:
            return b""
        value, delay = self.fragments.pop(0)
        if self.clock is not None:
            if delay >= self.timeouts[-1]:
                self.clock.now += self.timeouts[-1]
                raise TimeoutError("fixture socket receive timeout")
            self.clock.now += delay
        block, remaining = value[:maximum], value[maximum:]
        if remaining:
            self.fragments.insert(0, (remaining, 0.0))
        return block
    def close(self):
        self.closed = True


class _Connection:
    def __init__(self, fragments, clock=None):
        self.sock, self.closed, self.requests = _WireSocket(fragments, clock), False, []
    def request(self, method, target, headers):
        self.requests.append((method, target, headers))
    def close(self):
        self.closed = True
        self.sock.close()


def _wire(body=b"{}", headers=b"", status=200):
    return b"HTTP/1.1 " + str(status).encode() + b" Fixture\r\nContent-Type: application/json\r\n" + headers + b"\r\n" + body


def _fetch_wire(fragments, clock=None):
    connection = _Connection(fragments, clock)
    with patch.object(gdc, "_validate_public_dns", return_value=("fixture",)), patch.object(gdc, "_PinnedHttpsConnection", return_value=connection):
        return gdc._builtin_fetch(gdc._url(), 1000 if clock is not None else 30_000), connection


class ReviewedGdcGeneMutationRetrievalTests(unittest.TestCase):
    def test_fixed_config_plan_and_distinct_injected_transport_are_digest_bound(self):
        config = gdc.ReviewedGdcGeneMutationRetrievalConfig(**_TRANSPORT)
        self.assertEqual(config.request_limit, 1)
        self.assertEqual(config.to_dict()["gene_id"], "ENSG00000141510")
        self.assertEqual(config.to_dict()["project_ids"], ["TCGA-GBM", "TCGA-LGG"])
        self.assertEqual(gdc.ReviewedGdcGeneMutationRetrievalConfig.from_dict(config.to_dict()), config)
        plan = gdc.ReviewedGdcGeneMutationRetrievalPlan.create(config)
        self.assertEqual(gdc.ReviewedGdcGeneMutationRetrievalPlan.from_dict(plan.to_dict()).to_dict(), plan.to_dict())
        for timeout in (True, 0, 99, 30_001, None):
            with self.subTest(timeout=timeout), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                gdc.ReviewedGdcGeneMutationRetrievalConfig(timeout_ms=timeout)
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc.ReviewedGdcGeneMutationRetrievalAdapter(gdc.ReviewedGdcGeneMutationRetrievalConfig(), fetch=lambda _url: {})
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc.ReviewedGdcGeneMutationRetrievalAdapter(config)
        value = config.to_dict()
        value["gene_id"] = "ENSG00000138413"
        value["config_digest"] = content_digest({key: item for key, item in value.items() if key != "config_digest"})
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc.ReviewedGdcGeneMutationRetrievalConfig.from_dict(value)
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            replace(plan, plan_digest="0" * 64).validate()

    def test_literal_approval_and_plan_binding_hold_before_transport_or_dns(self):
        adapter, calls = _adapter()
        for approval in (False, 1, None, "yes"):
            with self.subTest(approval=approval), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                adapter.execute(adapter.prepare(), approve_source_dispatch=approval)
        different = gdc.ReviewedGdcGeneMutationRetrievalPlan.create(replace(adapter.config, timeout_ms=100))
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            adapter.execute(different, approve_source_dispatch=True)
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at="2026-02-30T00:00:00Z")
        self.assertEqual(calls, [])
        builtin = gdc.ReviewedGdcGeneMutationRetrievalAdapter(gdc.ReviewedGdcGeneMutationRetrievalConfig())
        with patch.object(gdc, "_validate_public_dns") as dns, self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            builtin.execute(builtin.prepare(), approve_source_dispatch=False)
        dns.assert_not_called()

    def test_one_exact_query_preserves_source_zero_and_explicit_other_project_omissions(self):
        adapter, calls = _adapter()
        result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_AT)
        self.assertEqual(len(calls), 1)
        parsed = urlsplit(calls[0])
        self.assertEqual((parsed.scheme, parsed.netloc, parsed.path), ("https", "api.gdc.cancer.gov", "/analysis/top_cases_counts_by_genes"))
        self.assertEqual(parse_qs(parsed.query), {"gene_ids": ["ENSG00000141510"]})
        bundle = result.bundle
        self.assertEqual([row["case_count_with_mutation"] for row in bundle["projects"]], [10, 0])
        self.assertEqual(bundle["observed_count"], 2)
        self.assertEqual(bundle["completeness"], "requested_counts_present")
        self.assertEqual(bundle["aggregation_metadata"]["omitted_non_glioma_project_bucket_count"], 1)
        self.assertNotIn("TCGA-BRCA", json.dumps(bundle))
        self.assertIsNone(bundle["denominator"])
        self.assertFalse(bundle["prevalence_calculated"])
        self.assertFalse(bundle["counts_pooled"])
        self.assertEqual(bundle["cohort_overlap"], "unassessed")
        self.assertEqual(result.receipt["request_count"], 1)
        self.assertEqual(result.receipt["network"], "caller_transport")
        self.assertEqual(gdc.validate_reviewed_gdc_gene_mutation_bundle(bundle), bundle)

    def test_absent_project_and_gene_buckets_stay_unknown_while_present_zero_stays_observed(self):
        response = _response()
        response["aggregations"]["projects"]["buckets"] = [_bucket("TCGA-GBM", 0, missing_gene=True)]
        bundle = _execute(response).bundle
        self.assertEqual([row["state"] for row in bundle["projects"]], ["gene_bucket_missing", "project_bucket_missing"])
        self.assertEqual([row["case_count_with_mutation"] for row in bundle["projects"]], [None, None])
        self.assertIsNone(bundle["projects"][1]["project_doc_count"])
        self.assertEqual(bundle["observed_count"], 0)
        self.assertEqual(bundle["completeness"], "missing_requested_counts")
        response["aggregations"]["projects"]["buckets"] = []
        self.assertTrue(all(row["state"] == "project_bucket_missing" for row in _execute(response).bundle["projects"]))

    def test_result_and_validated_bundle_copies_do_not_alias_internal_metadata(self):
        result = _execute()
        original = result.bundle
        result.bundle["projects"][0]["case_count_with_mutation"] = 999
        result.receipt["limitations"].append("altered")
        value = gdc.validate_reviewed_gdc_gene_mutation_bundle(original)
        value["projects"].clear()
        self.assertEqual(result.bundle, original)
        self.assertNotIn("altered", result.receipt["limitations"])
        self.assertNotIn("bundle", result.to_dict())
        self.assertEqual(result.to_transient_dict()["bundle"], original)

    def test_individual_records_unknown_fields_duplicate_identities_and_foreign_genes_fail(self):
        responses = []
        value = _response(); value["hits"]["hits"] = [{"case_id": "forbidden"}]; responses.append(value)
        value = _response(); value["patients"] = []; responses.append(value)
        value = _response(); value["aggregations"]["projects"]["buckets"][0]["sampleIds"] = []; responses.append(value)
        value = _response(); value["aggregations"]["projects"]["buckets"].append(deepcopy(value["aggregations"]["projects"]["buckets"][0])); responses.append(value)
        value = _response(); value["aggregations"]["projects"]["buckets"][0]["genes"]["my_genes"]["gene_id"]["buckets"][0]["key"] = "ENSG00000138413"; responses.append(value)
        value = _response(); value["aggregations"]["projects"]["buckets"][0]["genes"]["my_genes"]["gene_id"]["buckets"].append({"key": gdc.REVIEWED_GDC_GENE_MUTATION_GENE_ID, "doc_count": 10}); responses.append(value)
        for index, response in enumerate(responses):
            with self.subTest(index=index), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _execute(response)

    def test_partial_shards_timeouts_approximate_totals_and_error_bounds_cannot_become_exact_counts(self):
        for target, key, item in (("root", "timed_out", True), ("shards", "failed", 1), ("shards", "successful", 11), ("projects", "doc_count_error_upper_bound", 1), ("projects", "sum_other_doc_count", 1), ("genes", "doc_count_error_upper_bound", 1), ("genes", "sum_other_doc_count", 1), ("total", "relation", "gte")):
            response = _response()
            branches = {"root": response, "shards": response["_shards"], "projects": response["aggregations"]["projects"], "genes": response["aggregations"]["projects"]["buckets"][0]["genes"]["my_genes"]["gene_id"], "total": response["hits"]["total"]}
            branches[target][key] = item
            with self.subTest(target=target, key=key), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _execute(response)
        response = _response(); del response["_shards"]["failed"]
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _execute(response)

    def test_bool_float_null_negative_and_inconsistent_count_metadata_are_rejected(self):
        for invalid in (True, 10.0, None, -1, 1_000_000_000_001):
            response = _response()
            response["aggregations"]["projects"]["buckets"][0]["genes"]["my_genes"]["gene_id"]["buckets"][0]["doc_count"] = invalid
            with self.subTest(invalid=invalid), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _execute(response)
        response = _response(); response["aggregations"]["projects"]["buckets"][0]["genes"]["my_genes"]["doc_count"] = 11
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _execute(response)

    def test_duplicate_json_fields_bad_unicode_nonfinite_and_structural_byte_limits_fail_closed(self):
        for payload in ('{"took":1,"took":2}', '{"took":NaN}', '{"key":"\\ud800"}', b'\xff', 'x' * 1_000_001, []):
            with self.subTest(kind=type(payload).__name__), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _execute(payload)
        value = {}; cursor = value
        for _ in range(33):
            cursor["nested"] = {}; cursor = cursor["nested"]
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _execute(value)
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _execute({"nodes": [0] * 50_001})
        response = _response(); response["aggregations"]["projects"]["buckets"] = [_bucket("PROJECT-" + str(index), 0) for index in range(129)]
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _execute(response)

    def test_bundle_validation_rejects_restamped_boundary_count_pointer_and_missingness_drift(self):
        original = _execute().bundle
        for mutation in ("authority", "count", "pointer", "missing", "omitted", "extra"):
            bundle = deepcopy(original)
            if mutation == "authority": bundle["scientific_support_established"] = True
            elif mutation == "count": bundle["projects"][0]["case_count_with_mutation"] = True
            elif mutation == "pointer": bundle["projects"][0]["source_json_pointer"] = "/case_id"
            elif mutation == "missing": bundle["projects"][0]["state"] = "project_bucket_missing"
            elif mutation == "omitted": bundle["aggregation_metadata"]["returned_project_bucket_count"] += 1
            else: bundle["projects"][0]["patient_id"] = "forbidden"
            bundle["source_set_digest"] = content_digest(bundle["sources"])
            bundle["bundle_digest"] = content_digest({key: item for key, item in bundle.items() if key != "bundle_digest"})
            with self.subTest(mutation=mutation), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                gdc.validate_reviewed_gdc_gene_mutation_bundle(bundle)

    def test_source_failure_is_not_an_empty_result_and_is_not_retried(self):
        calls = []
        def fail(url):
            calls.append(url)
            raise OSError("fixture transport failed")
        adapter = gdc.ReviewedGdcGeneMutationRetrievalAdapter(gdc.ReviewedGdcGeneMutationRetrievalConfig(**_TRANSPORT), fetch=fail)
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True)
        self.assertEqual(len(calls), 1)

    def test_dns_rejects_private_mixed_or_empty_answers_before_connect(self):
        public = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("8.8.8.8", 443))
        private = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("127.0.0.1", 443))
        multicast = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("224.0.0.1", 443))
        for answers in ([], [private], [public, private], [multicast]):
            with self.subTest(answers=answers), patch.object(gdc.socket, "getaddrinfo", return_value=answers), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                gdc._validate_public_dns(time.monotonic() + 1)
        with patch.object(gdc.socket, "getaddrinfo", return_value=[public]):
            self.assertEqual(gdc._validate_public_dns(time.monotonic() + 1), public)

    def test_dns_failure_or_expired_deadline_cannot_dispatch_an_http_request(self):
        with patch.object(gdc.socket, "getaddrinfo", side_effect=OSError("fixture DNS failure")), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc._validate_public_dns(time.monotonic() + 1)
        with patch.object(gdc, "_PinnedHttpsConnection") as connection, self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc._remaining(time.monotonic() - 1)
        connection.assert_not_called()

    def test_tls_socket_connects_only_to_checked_address_and_authenticates_fixed_hostname(self):
        address = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("8.8.8.8", 443))
        connection = gdc._PinnedHttpsConnection(address, time.monotonic() + 1)
        raw, secured, context = Mock(), Mock(), Mock()
        context.wrap_socket.return_value = secured
        connection._context = context
        with patch.object(gdc.socket, "socket", return_value=raw), patch.object(gdc.socket, "getaddrinfo") as dns:
            connection.connect()
        raw.connect.assert_called_once_with(address[4])
        context.wrap_socket.assert_called_once_with(raw, server_hostname="api.gdc.cancer.gov")
        dns.assert_not_called()
        self.assertIs(connection.sock, secured)
        connection.close()

    def test_builtin_transport_uses_one_pinned_request_and_rejects_redirect_encoding_and_size_drift(self):
        body = json.dumps(_response()).encode()
        for status, headers, payload in ((200, b"", body), (302, b"Location: https://example.com\r\n", body), (200, b"Content-Type: text/html\r\n", body), (200, b"Content-Encoding: gzip\r\n", body), (200, b"Content-Length: 1000001\r\n", body), (200, b"Content-Length: " + str(len(body) + 1).encode() + b"\r\n", body), (200, b"", b'x' * 1_000_001)):
            connection = _Connection([_wire(payload, headers, status)])
            with self.subTest(status=status, headers=headers), patch.object(gdc, "_validate_public_dns", return_value=("fixture",)), patch.object(gdc, "_PinnedHttpsConnection", return_value=connection):
                if status == 200 and not headers and payload == body:
                    self.assertEqual(gdc._builtin_fetch(gdc._url(), 30_000), body)
                else:
                    with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                        gdc._builtin_fetch(gdc._url(), 30_000)
            self.assertEqual(len(connection.requests), 1)
            self.assertTrue(connection.closed)
            self.assertTrue(connection.sock.closed)
            self.assertEqual(connection.requests[0][0], "GET")
            self.assertEqual(connection.requests[0][2]["Connection"], "close")
        with patch.object(gdc, "_validate_public_dns") as dns, self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc._builtin_fetch("https://api.gdc.cancer.gov/cases", 30_000)
        dns.assert_not_called()

    def test_real_wire_parser_accepts_exact_length_and_strict_chunked_body(self):
        body = json.dumps(_response()).encode()
        received, _ = _fetch_wire([_wire(body, b"Content-Length: " + str(len(body)).encode() + b"\r\n")])
        self.assertEqual(received, body)
        chunked = b"1\r\n{\r\n1\r\n}\r\n0\r\n\r\n"
        received, _ = _fetch_wire([_wire(chunked, b"Transfer-Encoding: chunked\r\n")])
        self.assertEqual(received, b"{}")

    def test_real_wire_parser_rejects_duplicate_headers_and_ambiguous_or_unsupported_framing(self):
        headers = (b"Content-Length: 2\r\ncontent-length: 2\r\n", b"Content-Length: 2\r\nContent-Length: 9\r\n", b"Transfer-Encoding: chunked\r\ntransfer-encoding: chunked\r\n", b"Content-Length: 2\r\nTransfer-Encoding: chunked\r\n", b"Content-Length: 2, 2\r\n", b"Transfer-Encoding: gzip, chunked\r\n", b"Transfer-Encoding: identity\r\n", b"X-Header: value\r\n folded\r\n", b"Bad Header: value\r\n", b"Trailer: Digest\r\n")
        for value in headers:
            with self.subTest(headers=value), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _fetch_wire([_wire(headers=value)])
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _fetch_wire([b"HTTP/1.1 200 OK\nContent-Type: application/json\n\n{}"])
        with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            _fetch_wire([_wire(b"2\r\n{}\r\n0\r\n\r\n", b"Transfer-Encoding: chunked\r\n").replace(b"HTTP/1.1", b"HTTP/1.0", 1)])

    def test_real_wire_parser_rejects_malformed_chunk_delimiters_extensions_and_trailers(self):
        bodies = (b"2\r\n{}XX0\r\n\r\n", b"2;name=value\r\n{}\r\n0\r\n\r\n", b"2\n{}\r\n0\r\n\r\n", b"0\r\nX-Changed: value\r\n\r\n", b"0\r\n", b"f4241\r\n")
        for body in bodies:
            with self.subTest(body=body), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                _fetch_wire([_wire(body, b"Transfer-Encoding: chunked\r\n")])

    def test_real_wire_header_and_chunk_framing_work_are_bounded_independently_from_body(self):
        headers = b"First: " + b"x" * 33_000 + b"\r\nSecond: " + b"y" * 33_000 + b"\r\n"
        with self.assertRaisesRegex(gdc.ReviewedGdcGeneMutationRetrievalError, "headers exceed"):
            _fetch_wire([_wire(headers=headers)])
        chunked = b"1\r\nx\r\n" * 14_000 + b"0\r\n\r\n"
        with self.assertRaisesRegex(gdc.ReviewedGdcGeneMutationRetrievalError, "framing exceeds"):
            _fetch_wire([_wire(chunked, b"Transfer-Encoding: chunked\r\n")])

    def test_real_wire_slow_header_and_body_trickles_cannot_extend_the_absolute_deadline(self):
        for fragments in ([(bytes([item]), 0.3) for item in _wire()], [(_wire(b"", b"Content-Length: 10\r\n"), 0.0)] + [(b"x", 0.3)] * 10):
            clock = _Clock()
            connection = _Connection(fragments, clock)
            with self.subTest(header=fragments[0][1] > 0), patch.object(gdc.time, "monotonic", side_effect=clock.monotonic), patch.object(gdc, "_validate_public_dns", return_value=("fixture",)), patch.object(gdc, "_PinnedHttpsConnection", return_value=connection), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                gdc._builtin_fetch(gdc._url(), 1000)
            self.assertLessEqual(clock.now, 1.0)
            self.assertLessEqual(connection.sock.recv_calls, 5)
            self.assertLess(connection.sock.timeouts[-1], connection.sock.timeouts[0])
            self.assertTrue(connection.closed)

    def test_timed_out_dns_jobs_retain_four_slots_and_late_completion_cannot_dispatch(self):
        slots, release = threading.BoundedSemaphore(4), threading.Event()
        workers, entered = [], []
        thread_type = threading.Thread
        public = (socket.AF_INET, socket.SOCK_STREAM, 6, "", ("8.8.8.8", 443))
        def blocked_lookup(*_args, **_kwargs):
            entered.append(True)
            release.wait(1)
            return [public]
        def worker(*args, **kwargs):
            thread = thread_type(*args, **kwargs)
            workers.append(thread)
            return thread
        with patch.object(gdc, "_DNS_SLOTS", slots), patch.object(gdc.socket, "getaddrinfo", side_effect=blocked_lookup), patch.object(gdc.threading, "Thread", side_effect=worker), patch.object(gdc, "_PinnedHttpsConnection") as connection:
            try:
                for _ in range(5):
                    with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                        gdc._builtin_fetch(gdc._url(), 10)
                self.assertEqual(len(workers), 4)
                self.assertEqual(len(entered), 4)
                self.assertFalse(slots.acquire(blocking=False))
            finally:
                release.set()
                for thread in workers:
                    thread.join(timeout=1)
            connection.assert_not_called()
            self.assertTrue(all(not thread.is_alive() for thread in workers))
            for _ in range(4):
                self.assertTrue(slots.acquire(blocking=False))
            for _ in range(4):
                slots.release()

    def test_dns_worker_start_failure_releases_reserved_capacity(self):
        slots = threading.BoundedSemaphore(1)
        with patch.object(gdc, "_DNS_SLOTS", slots), patch.object(gdc.threading.Thread, "start", side_effect=RuntimeError("fixture start failure")), self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc._validate_public_dns(time.monotonic() + 1)
        self.assertTrue(slots.acquire(blocking=False))
        slots.release()

    def test_optional_catalog_registration_enforces_real_dispatch_schema_and_preserves_provider_count(self):
        from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
        baseline = builtin_workflow_operation_catalog()
        before = baseline.to_dict()
        catalog = gdc.register_gdc_gene_mutation_operations(baseline)
        self.assertEqual(catalog.to_dict()["operation_count"], before["operation_count"] + 1)
        self.assertEqual(catalog.to_dict()["integration_count"], before["integration_count"])
        self.assertNotIn("gdc.gene_mutation_counts", [operation.operation_id for operation in builtin_workflow_operation_catalog().operations()])
        self.assertEqual(catalog.validate_arguments("gdc.gene_mutation_counts", {}), {"timeout_ms": 30_000})
        with patch.object(gdc, "_builtin_fetch", return_value=_response()) as fetch:
            with self.assertRaises(ArgumentError):
                catalog.execute("gdc.gene_mutation_counts", {})
            fetch.assert_not_called()
            result = catalog.execute("gdc.gene_mutation_counts", {}, approve_source_dispatch=True, retrieved_at=_AT)
            self.assertEqual(result.value["projects"][1]["case_count_with_mutation"], 0)
            fetch.assert_called_once()
            response = _response(); response["aggregations"]["projects"]["buckets"] = []
            fetch.return_value = response
            self.assertIsNone(catalog.execute("gdc.gene_mutation_counts", {}, approve_source_dispatch=True).value["projects"][0]["case_count_with_mutation"])


if __name__ == "__main__":
    unittest.main()
