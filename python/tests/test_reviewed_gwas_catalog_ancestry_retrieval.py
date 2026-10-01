from __future__ import annotations

from copy import deepcopy
import json
from urllib.parse import urlsplit

import pytest

from prism_sdk import (
    REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT,
    ReviewedGwasCatalogAncestryRetrievalAdapter,
    ReviewedGwasCatalogAncestryRetrievalConfig,
    ReviewedGwasCatalogAncestryRetrievalPlan,
    ReviewedGwasCatalogRetrievalError,
    create_reviewed_gwas_catalog_ancestry_autonomous_evidence_registration,
    create_reviewed_gwas_catalog_ancestry_execution_metadata,
)


TRANSPORT = {"transport_id": "fixture.gwas-catalog-ancestry", "transport_version": "1", "transport_config_digest": "1" * 64}
ACCESSIONS = ("GCST90296481", "GCST000854")


def ancestry(accession: str, item_id: int, stage: str, individuals: int, group: str, countries: list[dict] | None) -> dict:
    return {
        "type": stage,
        "number_of_individuals": individuals,
        "ancestral_groups": [{"ancestral_group": group}],
        "country_of_origin": [],
        "country_of_recruitment": countries,
        "_links": {"self": {"href": f"{REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/{accession}/ancestries/{item_id}"}},
    }


def fixture(accession: str) -> dict:
    if accession == "GCST000854":
        return {"_embedded": {"ancestries": [
            ancestry(accession, 4595, "initial", 4390, "European", [{"major_area": "Europe", "region": "Northern Europe", "country_name": "U.K."}]),
            ancestry(accession, 7528, "replication", 2698, "European", [{"major_area": "Europe", "region": "Western Europe", "country_name": "Germany"}]),
            ancestry(accession, 7529, "replication", 1649, "NR", []),
        ]}}
    return {"_embedded": {"ancestries": [
        ancestry(accession, 165312141, "initial", 7273, "European", [
            {"major_area": "Europe", "region": "Northern Europe", "country_name": "Sweden"},
            {"major_area": "Northern America", "country_name": "U.S."},
            {"major_area": "Europe", "region": "Northern Europe", "country_name": "Denmark"},
        ]),
    ]}}


def config(studies=ACCESSIONS, **kwargs):
    return ReviewedGwasCatalogAncestryRetrievalConfig(studies, timeout_ms=1_500, **TRANSPORT, **kwargs)


class Transport:
    def __init__(self, corrupt: str | None = None):
        self.urls: list[str] = []
        self.corrupt = corrupt

    def __call__(self, url: str, timeout_ms: int):
        self.urls.append(url)
        assert timeout_ms == 1_500
        parsed = urlsplit(url)
        assert parsed.scheme == "https" and parsed.netloc == "www.ebi.ac.uk"
        accession = parsed.path.removeprefix("/gwas/rest/api/v2/studies/").removesuffix("/ancestries")
        assert parsed.path == f"/gwas/rest/api/v2/studies/{accession}/ancestries"
        assert accession in ACCESSIONS
        result = fixture(accession)
        rows = result["_embedded"]["ancestries"]
        if self.corrupt == "foreign_link":
            rows[0]["_links"]["self"]["href"] = "https://example.invalid/studies/elsewhere/ancestries/1"
        elif self.corrupt == "duplicate_id":
            rows.append(deepcopy(rows[0]))
        elif self.corrupt == "bad_count":
            rows[0]["number_of_individuals"] = -1
        elif self.corrupt == "duplicate_json":
            encoded = json.dumps(result, separators=(",", ":"))
            return encoded.replace('"type":"initial"', '"type":"initial","type":"initial"', 1)
        elif self.corrupt == "unknown_shape":
            result["unexpected"] = True
        return result


def test_config_plan_and_two_study_collection_are_canonical_and_bounded():
    transport = Transport()
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config(), fetch=transport)
    plan = adapter.prepare()
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at="2026-09-28T12:00:00Z")
    assert plan.config.study_accessions == ("GCST000854", "GCST90296481")
    assert transport.urls == [f"{REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/{study}/ancestries" for study in plan.config.study_accessions]
    assert result.receipt["request_count"] == 2
    assert result.bundle["study_count"] == 2
    assert result.bundle["ancestry_count"] == 4
    assert result.bundle["coverage"] == "all_source_rows"
    assert result.bundle["studies"][0]["ancestries"][2]["country_of_recruitment"] == []
    assert ReviewedGwasCatalogAncestryRetrievalConfig.from_dict(adapter.config.to_dict()) == adapter.config
    assert ReviewedGwasCatalogAncestryRetrievalPlan.from_dict(plan.to_dict()) == plan
    assert plan.to_dict()["maximum_source_ancestry_records_per_study"] == 500
    assert result.receipt["plan_digest"] == plan.plan_digest
    assert len(result.bundle["bundle_digest"]) == 64
    assert plan.config_digest == "7204e82b206e09a120d53e87f64f2ca785974400d9f1bddb145849144d70ff88"
    assert plan.plan_digest == "7f0bce4085cf1476b555c9121431b003d1fc70cbe66782f54078312dd26746b3"
    assert result.bundle["bundle_digest"] == "a89f598453cd2e4f375ba6f1af93f0be5477d12eb5ba1adc41546543f008e7b1"
    assert result.receipt["receipt_digest"] == "053dbf749a3778684be67558b12d280d5594e35ed1f21303c5e886520598331d"


def test_literal_approval_precedes_network_and_accession_set_is_bounded():
    transport = Transport()
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config(), fetch=transport)
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="literal approval"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=1)
    assert transport.urls == []
    for invalid in ((), ("GCST000854", "GCST000854"), ("GCST1",), tuple(f"GCST{i:05d}" for i in range(11))):
        with pytest.raises(ReviewedGwasCatalogRetrievalError):
            config(invalid)
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="single-study"):
        create_reviewed_gwas_catalog_ancestry_autonomous_evidence_registration(adapter, adapter.prepare(), study_accession="GCST90296481")


@pytest.mark.parametrize("corrupt", ["foreign_link", "duplicate_id", "bad_count", "duplicate_json", "unknown_shape"])
def test_foreign_or_malformed_ancestry_responses_fail_closed(corrupt):
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config(("GCST000854",)), fetch=Transport(corrupt=corrupt))
    with pytest.raises(ReviewedGwasCatalogRetrievalError):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_missing_metadata_and_reported_empty_lists_remain_distinct():
    accession = "GCST90296481"
    row = ancestry(accession, 900, "initial", 100, "European", [])
    row.pop("number_of_individuals")
    row.pop("country_of_origin")
    row["ancestral_groups"] = None
    transport = lambda _url, _timeout: {"_embedded": {"ancestries": [row]}}
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config((accession,)), fetch=transport)
    projected = adapter.execute(adapter.prepare(), approve_source_dispatch=True).bundle["studies"][0]["ancestries"][0]
    assert projected["number_of_individuals"] is None
    assert projected["country_of_origin"] is None
    assert projected["ancestral_groups"] is None
    assert projected["country_of_recruitment"] == []


def test_empty_collection_is_a_measured_empty_result():
    accession = "GCST90296481"
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config((accession,)), fetch=lambda _url, _timeout: {"_embedded": {"ancestries": []}})
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    study = result.bundle["studies"][0]
    assert study["total_ancestry_records"] == 0
    assert study["coverage"] == "all_source_rows"
    assert study["ancestries"] == []


def test_more_than_the_output_cap_is_reported_as_a_source_order_prefix():
    accession = "GCST90296481"
    rows = [ancestry(accession, item_id, "initial", item_id, "European", []) for item_id in range(1, 53)]
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config((accession,)), fetch=lambda _url, _timeout: {"_embedded": {"ancestries": rows}})
    study = adapter.execute(adapter.prepare(), approve_source_dispatch=True).bundle["studies"][0]
    assert study["total_ancestry_records"] == 52
    assert study["returned_ancestry_records"] == 50
    assert study["omitted_ancestry_records"] == 2
    assert study["coverage"] == "bounded_source_order_prefix"
    assert [row["ancestry_id"] for row in study["ancestries"]][-1] == 50


def test_oversized_source_collection_fails_before_projection():
    accession = "GCST90296481"
    rows = [ancestry(accession, item_id, "initial", item_id, "European", []) for item_id in range(1, 502)]
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config((accession,)), fetch=lambda _url, _timeout: {"_embedded": {"ancestries": rows}})
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="source rows"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_autonomous_projection_retains_only_digests_and_rejects_changed_transient_values():
    accession = "GCST90296481"
    adapter = ReviewedGwasCatalogAncestryRetrievalAdapter(config((accession,)), fetch=lambda url, timeout: fixture(accession))
    plan = adapter.prepare()
    registration = create_reviewed_gwas_catalog_ancestry_autonomous_evidence_registration(adapter, plan, study_accession=accession)
    metadata = create_reviewed_gwas_catalog_ancestry_execution_metadata(plan, approve_source_dispatch=True, retrieved_at="2026-09-28T12:00:00Z")
    transient = registration.acquire({"request": {"source_id": f"gwas_catalog_ancestry_{accession}", "source_digest": plan.plan_digest, "metadata": metadata}})
    projected = registration.project(transient, {"requirement": {"label": "GWAS study ancestry context"}})
    assert transient["retention"] == "caller_owned_transient_study_ancestry_metadata"
    assert projected[0]["confidence"] is None
    assert "value_digest" in projected[0] and "source_digest" in projected[0]
    assert "ancestries" not in projected[0]
    forged = deepcopy(transient)
    forged["bundle"]["studies"][0]["ancestries"][0]["ancestry_id"] += 1
    with pytest.raises(ReviewedGwasCatalogRetrievalError, match="digests"):
        registration.project(forged, {"requirement": {"label": "GWAS study ancestry context"}})
