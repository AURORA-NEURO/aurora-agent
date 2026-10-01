from __future__ import annotations

from copy import deepcopy
import json
from urllib.parse import parse_qs, urlencode, urlsplit

import pytest

from prism_sdk.reviewed_gwas_catalog_retrieval import (
    REVIEWED_GWAS_CATALOG_ENDPOINT,
    ReviewedGwasCatalogRetrievalAdapter,
    ReviewedGwasCatalogRetrievalConfig,
    ReviewedGwasCatalogRetrievalError,
    ReviewedGwasCatalogRetrievalPlan,
    create_reviewed_gwas_catalog_autonomous_evidence_registration,
    create_reviewed_gwas_catalog_execution_metadata,
)


TRANSPORT = {
    "transport_id": "fixture.gwas-catalog",
    "transport_version": "1",
    "transport_config_digest": "1" * 64,
}
TRAITS = {"gbm": "MONDO_0018177", "glioma": "MONDO_0021042"}


def config(**kwargs):
    return ReviewedGwasCatalogRetrievalConfig(
        lanes=("gbm", "glioma"), page_size=2, max_pages=2, timeout_ms=1_500,
        **TRANSPORT, **kwargs,
    )


def association(lane: str, row_id: int) -> dict:
    trait = "glioblastoma" if lane == "gbm" else "glioma"
    title = "Glioblastoma" if lane == "gbm" else "Glioma"
    return {
        "association_id": row_id,
        "accession_id": "GCST00001",
        "pubmed_id": "12345678",
        "first_author": "Example A",
        "p_value": 0.125,
        "efo_traits": [{"efo_id": TRAITS[lane], "efo_trait": trait}],
        "reported_trait": [title],
        "mapped_genes": ["EGFR", "TP53"],
        "locations": ["7:55012345"],
    }


def page(lane: str, page_number: int, *, total: int = 5, foreign_next: bool = False) -> dict:
    disease_id = TRAITS[lane]
    page_size = 2
    total_pages = (total + page_size - 1) // page_size
    start = page_number * page_size
    end = min(total, start + page_size)
    offset = 100 if lane == "gbm" else 200
    rows = [association(lane, offset + index) for index in range(start, end)]

    def url(number: int) -> str:
        return f"{REVIEWED_GWAS_CATALOG_ENDPOINT}?{urlencode((('efo_id', disease_id), ('show_child_traits', 'false'), ('page', str(number)), ('size', str(page_size))))}"

    links = {"self": {"href": url(page_number)}}
    if total_pages:
        links.update({"first": {"href": url(0)}, "last": {"href": url(total_pages - 1)}})
        if page_number + 1 < total_pages:
            links["next"] = {"href": "https://example.invalid/escape" if foreign_next else url(page_number + 1)}
    result = {"_links": links, "page": {"size": page_size, "totalElements": total, "totalPages": total_pages, "number": page_number}}
    if rows:
        result["_embedded"] = {"associations": rows}
    return result


class Transport:
    def __init__(self, *, drift=False, foreign_next=False, corrupt=None):
        self.urls = []
        self.drift = drift
        self.foreign_next = foreign_next
        self.corrupt = corrupt

    def __call__(self, url, timeout_ms):
        self.urls.append(url)
        parsed = urlsplit(url)
        query = parse_qs(parsed.query, strict_parsing=True)
        assert parsed.scheme == "https"
        assert parsed.netloc == "www.ebi.ac.uk"
        assert parsed.path == "/gwas/rest/api/v2/associations"
        assert set(query) == {"efo_id", "show_child_traits", "page", "size"}
        assert query["show_child_traits"] == ["false"]
        assert timeout_ms == 1_500
        lane = next(key for key, value in TRAITS.items() if value == query["efo_id"][0])
        page_number = int(query["page"][0])
        total = 5 if not self.drift or page_number == 0 else 7
        value = page(lane, page_number, total=total, foreign_next=self.foreign_next)
        if self.corrupt == "foreign_trait" and value.get("_embedded"):
            value["_embedded"]["associations"][0]["efo_traits"][0]["efo_id"] = "MONDO_0000001"
        if self.corrupt == "bad_pvalue" and value.get("_embedded"):
            value["_embedded"]["associations"][0]["p_value"] = 1.5
        if self.corrupt == "zero_pvalue" and value.get("_embedded"):
            value["_embedded"]["associations"][0]["p_value"] = 0
        if self.corrupt == "duplicate_json" and value.get("_embedded"):
            encoded = json.dumps(value, separators=(",", ":"))
            return encoded.replace('"association_id":100,', '"association_id":100,"association_id":100,', 1)
        return value


def test_config_plan_and_result_digests_are_stable_for_shared_transport_identity():
    transport = Transport()
    adapter = ReviewedGwasCatalogRetrievalAdapter(config(), fetch=transport)
    plan = adapter.prepare()
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at="2026-09-28T12:00:00Z")
    assert len(transport.urls) == 4
    assert result.bundle["coverage"] == "bounded_page_prefix"
    assert result.bundle["association_count"] == 8
    assert [row["returned_associations"] for row in result.bundle["lanes"]] == [4, 4]
    assert [row["omitted_associations"] for row in result.bundle["lanes"]] == [1, 1]
    assert result.receipt["request_count"] == 4
    assert result.receipt["coverage"] == "bounded_page_prefix"
    assert plan.config_digest == "4488a7090aab257524fb91dce8ac7540a4da6b1b2b4d6cc8467a3f794600527d"
    assert plan.plan_digest == "c0a2eada1b2aaeb543d0f9b649d2d8c6a52aa8eeb7dc9afc5a0e6e323fade511"
    assert result.bundle["bundle_digest"] == "5cfdda6822b5c67eb962b9e2f93771814a260b07ba3e998903892e1101eac5fd"
    assert result.receipt["receipt_digest"] == "144dfce90e2780072b4b9376799d5d9c0913f0db417148955909cb05ebeaefeb"
    assert ReviewedGwasCatalogRetrievalConfig.from_dict(adapter.config.to_dict()) == adapter.config
    assert ReviewedGwasCatalogRetrievalPlan.from_dict(plan.to_dict()) == plan


def test_dispatch_requires_literal_approval_and_pages_stay_on_the_fixed_source():
    transport = Transport()
    adapter = ReviewedGwasCatalogRetrievalAdapter(config(), fetch=transport)
    plan = adapter.prepare()
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)
    assert transport.urls == []
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at="2026-09-28T12:00:00Z")
    assert all(url.startswith(REVIEWED_GWAS_CATALOG_ENDPOINT + "?") for url in transport.urls)
    assert [parse_qs(urlsplit(url).query)["page"][0] for url in transport.urls] == ["0", "1", "0", "1"]
    assert result.bundle["lanes"][0]["query_semantics"] == "exact_ontology_trait_direct_matches_only"


@pytest.mark.parametrize("corrupt", ["foreign_trait", "bad_pvalue", "duplicate_json"])
def test_foreign_traits_invalid_pvalues_and_duplicate_json_fail_closed(corrupt):
    adapter = ReviewedGwasCatalogRetrievalAdapter(config(), fetch=Transport(corrupt=corrupt))
    with pytest.raises(ReviewedGwasCatalogRetrievalError):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_same_origin_pagination_and_stable_totals_are_required():
    for transport in (Transport(foreign_next=True), Transport(drift=True)):
        adapter = ReviewedGwasCatalogRetrievalAdapter(config(), fetch=transport)
        with pytest.raises(ReviewedGwasCatalogRetrievalError):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_zero_pvalue_is_kept_distinct_from_a_positive_reported_value():
    adapter = ReviewedGwasCatalogRetrievalAdapter(config(), fetch=Transport(corrupt="zero_pvalue"))
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    row = result.bundle["lanes"][0]["associations"][0]
    assert row["p_value"] == 0
    assert row["p_value_state"] == "reported_zero_or_underflow"


def test_autonomous_evidence_registration_keeps_values_transient_and_emits_only_digests():
    lane_config = ReviewedGwasCatalogRetrievalConfig(
        lanes=("gbm",), page_size=2, max_pages=2, timeout_ms=1_500, **TRANSPORT,
    )
    transport = Transport()
    adapter = ReviewedGwasCatalogRetrievalAdapter(lane_config, fetch=transport)
    plan = adapter.prepare()
    registration = create_reviewed_gwas_catalog_autonomous_evidence_registration(adapter, plan, lane="gbm")
    metadata = create_reviewed_gwas_catalog_execution_metadata(plan, approve_source_dispatch=True, retrieved_at="2026-09-28T12:00:00Z")
    request = {"source_id": "gwas_catalog_gbm", "source_digest": plan.plan_digest, "metadata": metadata}
    transient = registration.acquire({"request": request})
    assert transient["retention"] == "caller_owned_transient_association_metadata"
    projected = registration.project(transient, {"requirement": {"label": "GBM genetic association metadata"}})
    assert len(projected) == 1
    assert projected[0]["confidence"] is None
    assert "value_digest" in projected[0] and "source_digest" in projected[0]
    assert "associations" not in projected[0]
    forged = deepcopy(transient)
    forged["bundle"]["lanes"][0]["associations"][0]["association_id"] += 1
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="digests"):
        registration.project(forged, {"requirement": {"label": "GBM genetic association metadata"}})
