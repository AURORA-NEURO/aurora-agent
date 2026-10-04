from __future__ import annotations

import json
from urllib.parse import parse_qs, urlsplit

import pytest

import prism_sdk
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.authoring import content_digest
from prism_sdk.reviewed_europe_pmc_retrieval import (
    BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_EUROPE_PMC_TRANSPORT_ID,
    MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES,
    REVIEWED_EUROPE_PMC_LANES,
    ReviewedEuropePmcRetrievalAdapter,
    ReviewedEuropePmcRetrievalConfig,
    ReviewedEuropePmcRetrievalError,
    ReviewedEuropePmcRetrievalPlan,
    create_reviewed_europe_pmc_autonomous_evidence_registration,
    create_reviewed_europe_pmc_execution_metadata,
)


_TRANSPORT = {
    "transport_id": "fixture.europepmc",
    "transport_version": "1",
    "transport_config_digest": "a" * 64,
}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"


def _publication(record_id: str = "24999217", title: str = "Europe PMC metadata fixture", **overrides: object) -> dict[str, object]:
    return {
        "id": record_id,
        "source": "MED",
        "pmid": record_id,
        "doi": "10.1016/j.aat.2014.02.001",
        "title": title,
        "authorString": "Ghorbani J, Dabir S, Givehchi G, Najafi M.",
        "journalTitle": "Acta Anaesthesiol Taiwan",
        "pubYear": "2014",
        "pubType": "review; journal article; case reports",
        "isOpenAccess": "N",
        "citedByCount": 11,
        "firstPublicationDate": "2014-03-01",
        **overrides,
    }


def _response(records: list[dict[str, object]], hit_count: int, next_cursor: str | None = None) -> dict[str, object]:
    result: dict[str, object] = {"version": "6.9", "hitCount": hit_count, "request": {"queryString": "fixed", "resultType": "lite"}, "resultList": {"result": records}}
    if next_cursor is not None:
        result["nextCursorMark"] = next_cursor
    return result


def _adapter(responses: list[object], *, lanes: tuple[str, ...] = ("glioma",), **options: object):
    calls: list[str] = []

    def fetch(url: str) -> object:
        calls.append(url)
        return responses[len(calls) - 1]

    config = ReviewedEuropePmcRetrievalConfig(
        lanes=lanes,
        page_size=options.get("page_size", 2),
        max_pages=options.get("max_pages", 2),
        **_TRANSPORT,
    )
    return config, ReviewedEuropePmcRetrievalAdapter(config, fetch=fetch), calls


def test_preflight_is_pure_digest_bound_and_restricted_to_fixed_lanes() -> None:
    assert prism_sdk.ReviewedEuropePmcRetrievalAdapter is ReviewedEuropePmcRetrievalAdapter
    assert len(REVIEWED_EUROPE_PMC_LANES) == 6
    with pytest.raises(TypeError):
        REVIEWED_EUROPE_PMC_LANES["glioma"] = "caller query"  # type: ignore[index]
    config = ReviewedEuropePmcRetrievalConfig(lanes=("glioma",), **_TRANSPORT)
    adapter = ReviewedEuropePmcRetrievalAdapter(config, fetch=lambda _url: {})
    first = adapter.prepare()
    assert adapter.prepare().to_dict() == first.to_dict()
    assert ReviewedEuropePmcRetrievalConfig.from_dict(config.to_dict()) == config
    assert ReviewedEuropePmcRetrievalPlan.from_dict(first.to_dict()).to_dict() == first.to_dict()
    assert first.to_dict()["query_set_digest"] == config.to_dict()["query_set_digest"]
    assert first.to_dict()["request_limit"] == 2
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="unsupported or duplicate"):
        ReviewedEuropePmcRetrievalConfig(lanes=("cardiology",), **_TRANSPORT)
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="unsupported or duplicate"):
        ReviewedEuropePmcRetrievalConfig(lanes=("glioma", "glioma"), **_TRANSPORT)
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="invalid shape"):
        ReviewedEuropePmcRetrievalConfig.from_dict({**config.to_dict(), "arbitrary_query": "glioma"})


def test_builtin_transport_identity_is_distinct_from_injected_transport() -> None:
    builtin = ReviewedEuropePmcRetrievalConfig()
    assert builtin.transport_id == BUILTIN_EUROPE_PMC_TRANSPORT_ID
    assert builtin.transport_config_digest == BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="distinct reviewed identity"):
        ReviewedEuropePmcRetrievalAdapter(builtin, fetch=lambda _url: {})
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="not exact"):
        ReviewedEuropePmcRetrievalAdapter(ReviewedEuropePmcRetrievalConfig(**_TRANSPORT))


def test_execute_requires_literal_approval_and_persists_no_publication_text_or_query() -> None:
    _, adapter, calls = _adapter([_response([_publication()], 1)])
    plan = adapter.prepare()
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []

    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert result.bundle["publications"][0]["title"] == "Europe PMC metadata fixture"
    assert result.bundle["publications"][0]["is_open_access"] is False
    assert result.receipt["completeness"] == "complete"
    serialized = json.dumps(result.to_dict())
    assert "Europe PMC metadata fixture" not in serialized
    assert "glioblastoma OR" not in serialized
    assert "nextCursorMark" not in serialized
    detached = result.bundle
    detached["publications"][0]["title"] = "changed"
    assert result.bundle["publications"][0]["title"] == "Europe PMC metadata fixture"


def test_cursor_pagination_is_rebuilt_on_pinned_endpoint_and_reports_partial_coverage() -> None:
    cursor = "cursor/+=="
    _, adapter, calls = _adapter([
        _response([_publication()], 3, cursor),
        _response([_publication("PMC4054321", "second result", pmid=None, pmcid="PMC4054321", doi=None)], 3),
    ])
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert len(calls) == 2
    for index, value in enumerate(calls):
        parsed = urlsplit(value)
        query = parse_qs(parsed.query, strict_parsing=True)
        assert (parsed.scheme, parsed.netloc, parsed.path) == ("https", "www.ebi.ac.uk", "/europepmc/webservices/rest/search")
        assert query["format"] == ["json"]
        assert query["resultType"] == ["lite"]
        assert query["synonym"] == ["N"]
        assert query["pageSize"] == ["2"]
        assert query["query"] == [REVIEWED_EUROPE_PMC_LANES["glioma"]]
        assert query["cursorMark"] == (["*"] if index == 0 else [cursor])
    assert result.receipt["request_count"] == 2
    assert result.receipt["reported_hit_count"] == 3
    assert result.receipt["omitted_record_count"] == 1
    assert result.receipt["completeness"] == "partial"


def test_changing_hit_counts_remain_unknown_instead_of_becoming_zero_or_complete() -> None:
    _, adapter, _ = _adapter([
        _response([_publication()], 4, "next"),
        _response([_publication("PMC4054321", "second", pmid=None, pmcid="PMC4054321", doi=None)], 5),
    ])
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    source = result.receipt["source_receipts"][0]
    assert source["reported_hit_count"] is None
    assert source["omitted_record_count"] is None
    assert source["completeness"] == "unknown"
    assert result.receipt["reported_hit_count"] is None
    assert result.receipt["completeness"] == "unknown"


def test_duplicate_keys_bad_rows_repeated_cursors_and_non_finite_numbers_fail_closed() -> None:
    bad_inputs: list[list[object]] = [
        ['{"hitCount":0,"hitCount":0,"resultList":{"result":[]}}'],
        ['{"hitCount":0,"resultList":{"result":[]},"extra":1e400}'],
        [_response([_publication("not-an-id", pmid="not-an-id", doi=None)], 1)],
        [_response([_publication(pmid=None, doi=None)], 1)],
        [_response([_publication(), _publication()], 1)],
        [_response([], 3, "cursor-a"), _response([], 3, "cursor-b"), _response([], 3, "cursor-a")],
    ]
    for index, replies in enumerate(bad_inputs):
        _, adapter, _ = _adapter(replies, max_pages=4 if index == len(bad_inputs) - 1 else 2)
        with pytest.raises(ReviewedEuropePmcRetrievalError):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)


def test_response_byte_tree_and_bundle_bounds_are_enforced() -> None:
    _, adapter, _ = _adapter([" " * (MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES + 1)])
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="byte bound"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    nested: object = None
    for _ in range(45):
        nested = [nested]
    _, deep_adapter, _ = _adapter([{"hitCount": 0, "resultList": {"result": []}, "ignored": nested}])
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="tree bound"):
        deep_adapter.execute(deep_adapter.prepare(), approve_source_dispatch=True)
    cyclic: dict[str, object] = {}
    cyclic["self"] = cyclic
    _, cycle_adapter, _ = _adapter([cyclic])
    with pytest.raises(ReviewedEuropePmcRetrievalError):
        cycle_adapter.execute(cycle_adapter.prepare(), approve_source_dispatch=True)


def test_unicode_normalization_matches_python_and_unpaired_surrogates_fail_closed() -> None:
    _, adapter, _ = _adapter([_response([_publication(title=" \x1c\x85Europe\u2003PMC  ")], 1)])
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert result.bundle["publications"][0]["title"] == "Europe PMC"
    _, invalid, _ = _adapter(['{"hitCount":1,"resultList":{"result":[{"id":"1","source":"MED","pmid":"1","title":"\\ud800"}]}}'])
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="Unicode"):
        invalid.execute(invalid.prepare(), approve_source_dispatch=True)


def test_registration_binds_source_plan_and_projects_digest_only_evidence() -> None:
    config, adapter, _ = _adapter([_response([_publication()], 1)])
    plan = adapter.prepare()
    registration = create_reviewed_europe_pmc_autonomous_evidence_registration(adapter, plan, lane="glioma")
    registry = AutonomousEvidenceAdapterRegistry([registration])
    assert registry.manifests()[0].adapter_id == "reviewed.europepmc.glioma"
    metadata = create_reviewed_europe_pmc_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    transient = registration.acquire({"request": {"source_id": "europepmc_glioma", "source_digest": plan.plan_digest, "metadata": metadata}})
    observations = registration.project(transient, {"requirement": {"label": "literature"}})
    assert observations[0]["kind"] == "provenance"
    assert observations[0]["value_digest"] == transient["receipt"]["bundle_digest"]
    assert "Europe PMC metadata fixture" not in json.dumps(observations)
    forged = {**metadata, "approve_source_dispatch": False}
    with pytest.raises(ReviewedEuropePmcRetrievalError):
        registration.acquire({"request": {"source_id": "europepmc_glioma", "source_digest": plan.plan_digest, "metadata": forged}})

    tampered = json.loads(json.dumps(transient))
    tampered["bundle"]["publications"][0]["title"] = "fabricated"
    bundle_unsigned = {key: value for key, value in tampered["bundle"].items() if key != "bundle_digest"}
    tampered["bundle"]["bundle_digest"] = content_digest(bundle_unsigned)
    tampered["receipt"]["bundle_digest"] = tampered["bundle"]["bundle_digest"]
    receipt_unsigned = {key: value for key, value in tampered["receipt"].items() if key != "receipt_digest"}
    tampered["receipt"]["receipt_digest"] = content_digest(receipt_unsigned)
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="source metadata"):
        registration.project(tampered, {"requirement": {"label": "literature"}})


def test_transport_exception_details_are_redacted_and_are_not_retried() -> None:
    calls = 0

    def fail(_url: str) -> object:
        nonlocal calls
        calls += 1
        raise RuntimeError("private response body")

    config = ReviewedEuropePmcRetrievalConfig(**_TRANSPORT)
    adapter = ReviewedEuropePmcRetrievalAdapter(config, fetch=fail)
    with pytest.raises(ReviewedEuropePmcRetrievalError, match="request failed") as raised:
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    assert "private response body" not in str(raised.value)
    assert calls == 1
