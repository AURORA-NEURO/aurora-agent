"""Focused invariants for one reviewed aggregate GDC query; all tests stay offline."""
from copy import deepcopy
from dataclasses import replace
import json
import socket
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


class _Socket:
    def settimeout(self, value):
        if not 0 < value <= 30:
            raise AssertionError("socket deadline was not bounded")


class _Response:
    def __init__(self, body, status=200, headers=None):
        self.status, self.body = status, body
        self.headers = {"Content-Type": "application/json", **(headers or {})}
    def getheader(self, name, default=None):
        return self.headers.get(name, default)
    def read1(self, maximum):
        chunk, self.body = self.body[:maximum], self.body[maximum:]
        return chunk


class _Connection:
    def __init__(self, response):
        self.response, self.sock, self.closed, self.requests = response, _Socket(), False, []
    def request(self, method, target, headers):
        self.requests.append((method, target, headers))
    def getresponse(self):
        return self.response
    def close(self):
        self.closed = True


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
        for answers in ([], [private], [public, private]):
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
        for status, headers, payload in ((200, {}, body), (302, {"Location": "https://example.com"}, body), (200, {"Content-Type": "text/html"}, body), (200, {"Content-Encoding": "gzip"}, body), (200, {"Content-Length": "1000001"}, body), (200, {"Content-Length": str(len(body) + 1)}, body), (200, {}, b'x' * 1_000_001)):
            connection = _Connection(_Response(payload, status, headers))
            with self.subTest(status=status, headers=headers), patch.object(gdc, "_validate_public_dns", return_value=("fixture",)), patch.object(gdc, "_PinnedHttpsConnection", return_value=connection):
                if status == 200 and not headers and payload == body:
                    self.assertEqual(gdc._builtin_fetch(gdc._url(), 30_000), body)
                else:
                    with self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
                        gdc._builtin_fetch(gdc._url(), 30_000)
            self.assertEqual(len(connection.requests), 1)
            self.assertTrue(connection.closed)
            self.assertEqual(connection.requests[0][0], "GET")
        with patch.object(gdc, "_validate_public_dns") as dns, self.assertRaises(gdc.ReviewedGdcGeneMutationRetrievalError):
            gdc._builtin_fetch("https://api.gdc.cancer.gov/cases", 30_000)
        dns.assert_not_called()

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
