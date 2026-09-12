from __future__ import annotations

import json
import importlib
from urllib.parse import parse_qs, unquote, urlsplit

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.public_literature_refresh import (
    PUBMED_SPECIALTY_LANES,
    PublicLiteratureRefreshError,
    bundle_digest,
    refresh_neurosurgical_public_literature,
)
from prism_sdk.reviewed_pubmed_retrieval import (
    REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA,
    ReviewedPubMedRetrievalAdapter,
    ReviewedPubMedRetrievalConfig,
    ReviewedPubMedRetrievalError,
    ReviewedPubMedRetrievalPlan,
    ReviewedPubMedRetrievalReceipt,
    create_reviewed_pubmed_autonomous_evidence_registration,
    create_reviewed_pubmed_execution_metadata,
)


_FIXTURE_TRANSPORT_DIGEST = content_digest({"fixture": "deterministic-pubmed-fetch-v1"})
_RETRIEVED_AT = "2026-09-02T12:00:00Z"
_refresh_module = importlib.import_module("prism_sdk.public_literature_refresh")
_reviewed_module = importlib.import_module("prism_sdk.reviewed_pubmed_retrieval")


def _fixture_fetch(calls: list[str]):
    def fetch(url: str):
        calls.append(url)
        if "esearch.fcgi" in url:
            term = unquote(url.split("term=", 1)[1].split("&", 1)[0])
            lane = next(
                key for key, value in PUBMED_SPECIALTY_LANES.items() if value == term
            )
            pmid = str(20_000 + list(PUBMED_SPECIALTY_LANES).index(lane))
            return {"esearchresult": {"idlist": [pmid]}}
        pmid = url.split("id=", 1)[1].split("&", 1)[0]
        if "esummary.fcgi" in url:
            return {
                "result": {
                    pmid: {
                        "title": "Observed PubMed cohort",
                        "fulljournalname": "Journal of Neurosurgery",
                        "pubdate": "2026 Jan 02",
                        "articleids": [{"idtype": "doi", "value": "10.1000/aurora"}],
                    }
                }
            }
        return (
            '<?xml version="1.0" ?>'
            '<!DOCTYPE PubmedArticleSet PUBLIC "-//NLM//DTD PubMedArticle, 1st January 2025//EN" '
            '"https://dtd.nlm.nih.gov/ncbi/pubmed/out/pubmed_250101.dtd">'
            "<PubmedArticleSet><PubmedArticle><MedlineCitation>"
            f'<PMID>{pmid}</PMID><Article><Abstract><AbstractText Label="BACKGROUND">'
            "Observed source abstract</AbstractText></Abstract><PublicationTypeList>"
            "<PublicationType>Journal Article</PublicationType></PublicationTypeList></Article>"
            "<MeshHeadingList><MeshHeading><DescriptorName>Neurosurgery</DescriptorName>"
            "</MeshHeading></MeshHeadingList></MedlineCitation></PubmedArticle></PubmedArticleSet>"
        ).encode()

    return fetch


def _config(lanes=("glioma",), **overrides) -> ReviewedPubMedRetrievalConfig:
    values = {
        "specialty_lanes": lanes,
        "per_specialty_limit": 1,
        "transport_id": "fixture.pubmed.fetch",
        "transport_version": "1",
        "transport_config_digest": _FIXTURE_TRANSPORT_DIGEST,
    }
    values.update(overrides)
    return ReviewedPubMedRetrievalConfig(**values)


def test_allow_listed_ncbi_doctype_is_removed_before_the_base_parser() -> None:
    response = _fixture_fetch([])(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&id=20000&rettype=abstract&retmode=xml"
    )

    normalized, observed_bytes = _reviewed_module._bounded_response(
        response,
        endpoint="efetch.fcgi",
        byte_limit=100_000,
    )

    assert observed_bytes == len(response)
    assert isinstance(normalized, bytes)
    assert b"<!DOCTYPE" not in normalized
    assert normalized.startswith(b'<?xml version="1.0" ?>')


def test_preflight_is_pure_stable_and_contains_no_query_text() -> None:
    calls: list[str] = []
    adapter = ReviewedPubMedRetrievalAdapter(
        _config(("cranial_base", "glioma")),
        fetch=_fixture_fetch(calls),
    )
    first = adapter.prepare()
    second = adapter.prepare()

    assert calls == []
    assert first == second
    assert first.specialty_lanes == ("glioma", "cranial_base")
    assert first.request_limit == 6
    assert first.record_limit == 2
    assert ReviewedPubMedRetrievalPlan.from_dict(first.to_dict()) == first
    serialized = json.dumps(first.to_dict(), sort_keys=True)
    assert all(query not in serialized for query in PUBMED_SPECIALTY_LANES.values())
    assert "ready_for_review" in serialized


def test_execution_requires_literal_approval_and_rejects_plan_or_callable_drift_before_dispatch() -> (
    None
):
    calls: list[str] = []
    adapter = ReviewedPubMedRetrievalAdapter(_config(), fetch=_fixture_fetch(calls))
    plan = adapter.prepare()

    for approval in (False, 1, "true", None):
        with pytest.raises(ReviewedPubMedRetrievalError, match="literal approval"):
            adapter.execute(plan, approve_source_dispatch=approval)  # type: ignore[arg-type]
    assert calls == []

    other_plan = ReviewedPubMedRetrievalAdapter(
        _config(("chiari_malformation",)),
        fetch=_fixture_fetch([]),
    ).prepare()
    with pytest.raises(ReviewedPubMedRetrievalError, match="drifted"):
        adapter.execute(other_plan, approve_source_dispatch=True)
    assert calls == []

    object.__setattr__(adapter, "_fetch", _fixture_fetch([]))
    with pytest.raises(ReviewedPubMedRetrievalError, match="callable changed"):
        adapter.execute(plan, approve_source_dispatch=True)
    assert calls == []


def test_reviewed_query_digest_fences_catalogue_mutation_before_every_dispatch() -> (
    None
):
    original_term = PUBMED_SPECIALTY_LANES["glioma"]
    before_calls: list[str] = []
    before_adapter = ReviewedPubMedRetrievalAdapter(
        _config(), fetch=_fixture_fetch(before_calls)
    )
    before_plan = before_adapter.prepare()
    assert before_plan.query_set_digest == before_adapter.config.query_set_digest
    try:
        PUBMED_SPECIALTY_LANES["glioma"] = "unreviewed widened query"
        with pytest.raises(ReviewedPubMedRetrievalError, match="queries changed"):
            before_adapter.execute(before_plan, approve_source_dispatch=True)
        assert before_calls == []
    finally:
        PUBMED_SPECIALTY_LANES["glioma"] = original_term

    mid_calls: list[str] = []
    base_fetch = _fixture_fetch(mid_calls)

    def mutate_after_first_response(url: str):
        response = base_fetch(url)
        if len(mid_calls) == 1:
            PUBMED_SPECIALTY_LANES["glioma"] = "unreviewed widened query"
        return response

    mid_adapter = ReviewedPubMedRetrievalAdapter(
        _config(), fetch=mutate_after_first_response
    )
    try:
        with pytest.raises(ReviewedPubMedRetrievalError, match="queries changed"):
            mid_adapter.execute(mid_adapter.prepare(), approve_source_dispatch=True)
        assert len(mid_calls) == 1
    finally:
        PUBMED_SPECIALTY_LANES["glioma"] = original_term

    surface_calls: list[str] = []
    surface_adapter = ReviewedPubMedRetrievalAdapter(
        _config(), fetch=_fixture_fetch(surface_calls)
    )
    surface_plan = surface_adapter.prepare()
    original_builder = _refresh_module._pubmed_url
    try:
        _refresh_module._pubmed_url = lambda *_args, **_kwargs: "https://example.test/"
        with pytest.raises(
            ReviewedPubMedRetrievalError, match="implementation changed"
        ):
            surface_adapter.execute(surface_plan, approve_source_dispatch=True)
        assert surface_calls == []
    finally:
        _refresh_module._pubmed_url = original_builder


def test_selected_lane_execution_is_bounded_and_receipt_is_metadata_only() -> None:
    calls: list[str] = []
    adapter = ReviewedPubMedRetrievalAdapter(
        _config(("glioma", "cranial_base")),
        fetch=_fixture_fetch(calls),
    )
    plan = adapter.prepare()
    result = adapter.execute(
        plan,
        approve_source_dispatch=True,
        retrieved_at=_RETRIEVED_AT,
    )

    assert len(calls) == 6
    assert sum("esearch.fcgi" in call for call in calls) == 2
    assert all(
        term not in " ".join(calls)
        for lane, term in PUBMED_SPECIALTY_LANES.items()
        if lane not in plan.specialty_lanes
    )
    bundle = result.bundle
    assert {source["source_id"] for source in bundle["sources"]} == {
        "pubmed_glioma",
        "pubmed_cranial_base",
    }
    assert result.receipt.bundle_digest == bundle_digest(bundle)
    assert result.receipt.request_count == 6
    assert result.receipt.record_count == 2
    assert result.receipt.abstract_count == 2
    assert (
        ReviewedPubMedRetrievalReceipt.from_dict(result.receipt.to_dict())
        == result.receipt
    )
    receipt_text = json.dumps(result.receipt.to_dict(), sort_keys=True).lower()
    assert all(
        query.lower() not in receipt_text for query in PUBMED_SPECIALTY_LANES.values()
    )
    assert all(
        word not in receipt_text for word in ("uri", "body", "credential", "title")
    )
    assert "observed source abstract" not in receipt_text
    assert "observed source abstract" not in repr(result).lower()


def test_registered_ncbi_contact_is_digest_bound_on_every_request_and_redacted_from_artifacts() -> (
    None
):
    ncbi_tool = "aurora_registered_research"
    ncbi_email = "eutilities-contact@example.org"
    calls: list[str] = []
    config = _config(ncbi_tool=ncbi_tool, ncbi_email=ncbi_email)
    adapter = ReviewedPubMedRetrievalAdapter(config, fetch=_fixture_fetch(calls))
    plan = adapter.prepare()

    assert plan.ncbi_registration_configured is True
    assert plan.ncbi_registration_digest == config.ncbi_registration_digest
    assert (
        ReviewedPubMedRetrievalConfig.from_dict(
            config.to_dict(),
            ncbi_tool=ncbi_tool,
            ncbi_email=ncbi_email,
        )
        == config
    )
    with pytest.raises(ReviewedPubMedRetrievalError, match="contact pair"):
        ReviewedPubMedRetrievalConfig.from_dict(config.to_dict())

    result = adapter.execute(
        plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT
    )

    assert len(calls) == 3
    for url in calls:
        parameters = parse_qs(urlsplit(url).query, strict_parsing=True)
        assert parameters["tool"] == [ncbi_tool]
        assert parameters["email"] == [ncbi_email]
        assert "api_key" not in parameters
    assert result.receipt.ncbi_registration_configured is True
    assert result.receipt.ncbi_registration_digest == plan.ncbi_registration_digest
    assert (
        ReviewedPubMedRetrievalReceipt.from_dict(result.receipt.to_dict())
        == result.receipt
    )

    # The values must reach NCBI, but must not persist in review artifacts, receipts, reprs, or the
    # source URI embedded in the transient bundle.
    artifacts = json.dumps(
        {
            "config": config.to_dict(),
            "plan": plan.to_dict(),
            "receipt": result.receipt.to_dict(),
            "source_uri": result.bundle["sources"][0]["uri"],
        },
        sort_keys=True,
    )
    assert ncbi_tool not in artifacts
    assert ncbi_email not in artifacts
    assert ncbi_tool not in repr(config)
    assert ncbi_email not in repr(config)
    assert "ncbi_registration_digest" in artifacts


@pytest.mark.parametrize(
    ("overrides", "message"),
    [
        ({"ncbi_tool": "aurora"}, "provided together"),
        ({"ncbi_email": "developer@example.org"}, "provided together"),
        (
            {"ncbi_tool": "aurora research", "ncbi_email": "developer@example.org"},
            "without spaces",
        ),
        ({"ncbi_tool": "aurora", "ncbi_email": "not-an-email"}, "developer email"),
        (
            {
                "ncbi_tool": "aurora",
                "ncbi_email": "developer@example.org",
                "ncbi_registration_digest": "0" * 64,
            },
            "does not match",
        ),
    ],
)
def test_ncbi_registration_is_bounded_paired_and_not_a_secret_channel(
    overrides, message
) -> None:
    with pytest.raises(ReviewedPubMedRetrievalError, match=message):
        _config(**overrides)


def test_ncbi_registration_mutation_is_refused_before_source_dispatch() -> None:
    calls: list[str] = []
    config = _config(
        ncbi_tool="aurora_registered_research",
        ncbi_email="eutilities-contact@example.org",
    )
    adapter = ReviewedPubMedRetrievalAdapter(config, fetch=_fixture_fetch(calls))
    plan = adapter.prepare()

    object.__setattr__(config, "ncbi_email", "changed@example.org")
    with pytest.raises(ReviewedPubMedRetrievalError, match="contact pair changed"):
        adapter.execute(plan, approve_source_dispatch=True)
    assert calls == []


def test_response_bytes_depth_and_request_scope_are_fail_closed_without_fallback() -> (
    None
):
    oversized_calls: list[str] = []

    def oversized_fetch(url: str):
        oversized_calls.append(url)
        return {"esearchresult": {"idlist": ["1"], "padding": "x" * 300}}

    oversized = ReviewedPubMedRetrievalAdapter(
        _config(response_byte_limit=256, total_response_byte_limit=256),
        fetch=oversized_fetch,
    )
    with pytest.raises(ReviewedPubMedRetrievalError, match="byte (bound|limit)"):
        oversized.execute(oversized.prepare(), approve_source_dispatch=True)
    assert len(oversized_calls) == 1

    deep_calls: list[str] = []

    def deep_fetch(url: str):
        deep_calls.append(url)
        value: dict[str, object] = {"idlist": ["1"]}
        for _ in range(34):
            value = {"next": value}
        return {"esearchresult": value}

    deep = ReviewedPubMedRetrievalAdapter(_config(), fetch=deep_fetch)
    with pytest.raises(ReviewedPubMedRetrievalError, match="deeply nested"):
        deep.execute(deep.prepare(), approve_source_dispatch=True)
    assert len(deep_calls) == 1

    failed_calls: list[str] = []

    def failed_fetch(url: str):
        failed_calls.append(url)
        raise OSError("offline")

    failed = ReviewedPubMedRetrievalAdapter(_config(), fetch=failed_fetch)
    with pytest.raises(ReviewedPubMedRetrievalError, match="reviewed source contract"):
        failed.execute(failed.prepare(), approve_source_dispatch=True)
    assert len(failed_calls) == 1

    entity_calls: list[str] = []
    safe_fetch = _fixture_fetch(entity_calls)

    def entity_fetch(url: str):
        value = safe_fetch(url)
        if "efetch.fcgi" not in url:
            return value
        return (
            b'<!DOCTYPE PubmedArticleSet [<!ENTITY leak "forbidden">]>'
            b"<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>20000</PMID>"
            b"<Article><Abstract><AbstractText>&leak;</AbstractText></Abstract></Article>"
            b"</MedlineCitation></PubmedArticle></PubmedArticleSet>"
        )

    entity_adapter = ReviewedPubMedRetrievalAdapter(_config(), fetch=entity_fetch)
    with pytest.raises(
        ReviewedPubMedRetrievalError, match="forbidden document declaration"
    ):
        entity_adapter.execute(entity_adapter.prepare(), approve_source_dispatch=True)
    assert len(entity_calls) == 3


def test_exact_artifact_keys_and_digests_refuse_unknown_or_mutated_fields() -> None:
    config = _config()
    malformed_config = config.to_dict()
    malformed_config["query"] = "glioma"
    with pytest.raises(ReviewedPubMedRetrievalError, match="exactly"):
        ReviewedPubMedRetrievalConfig.from_dict(malformed_config)

    plan = ReviewedPubMedRetrievalAdapter(config, fetch=_fixture_fetch([])).prepare()
    malformed_plan = plan.to_dict()
    malformed_plan["record_limit"] = 2
    with pytest.raises(ReviewedPubMedRetrievalError):
        ReviewedPubMedRetrievalPlan.from_dict(malformed_plan)

    calls: list[str] = []
    adapter = ReviewedPubMedRetrievalAdapter(_config(), fetch=_fixture_fetch(calls))
    receipt = adapter.execute(
        adapter.prepare(),
        approve_source_dispatch=True,
        retrieved_at=_RETRIEVED_AT,
    ).receipt.to_dict()
    receipt["sources"][0]["record_count"] = 2
    with pytest.raises(ReviewedPubMedRetrievalError):
        ReviewedPubMedRetrievalReceipt.from_dict(receipt)


def test_generic_registration_executes_and_projects_only_its_single_reviewed_lane() -> (
    None
):
    calls: list[str] = []
    adapter = ReviewedPubMedRetrievalAdapter(_config(), fetch=_fixture_fetch(calls))
    plan = adapter.prepare()
    registration = create_reviewed_pubmed_autonomous_evidence_registration(
        adapter,
        plan,
        specialty_lane="glioma",
    )
    registry = AutonomousEvidenceAdapterRegistry((registration,))
    manifest = registry.resolve("biomedical", registration.adapter_id)
    assert manifest.manifest_digest
    assert "literature" in manifest.capabilities

    context = {
        "requirement": {"domain": "biomedical", "label": "reviewed_literature"},
        "request": {
            "source_id": "pubmed_glioma",
            "source_digest": plan.plan_digest,
            "metadata": create_reviewed_pubmed_execution_metadata(
                plan,
                approve_source_dispatch=True,
                retrieved_at=_RETRIEVED_AT,
            ),
        },
    }
    value = registry.create_acquirer({"biomedical": registration.adapter_id}).acquire(
        context
    )
    assert value["schema"] == REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA
    observations = registry.create_projector(
        {"biomedical": registration.adapter_id}
    ).project(value, context)
    assert observations == [
        {
            "label": "reviewed_literature",
            "kind": "provenance",
            "status": "observed",
            "value_digest": value["receipt"]["bundle_digest"],
            "source_digest": value["receipt"]["source_set_digest"],
            "confidence": None,
            "limitations": value["receipt"]["limitations"],
        }
    ]
    assert len(calls) == 3

    refused = dict(context)
    refused["request"] = dict(context["request"])
    refused["request"]["metadata"] = dict(context["request"]["metadata"])
    refused["request"]["metadata"]["approve_source_dispatch"] = 1
    with pytest.raises(ReviewedPubMedRetrievalError, match="literal approval"):
        registration.acquire(refused)
    assert len(calls) == 3

    multi_lane = ReviewedPubMedRetrievalAdapter(
        _config(("glioma", "cranial_base")),
        fetch=_fixture_fetch([]),
    )
    with pytest.raises(ReviewedPubMedRetrievalError, match="single-lane"):
        create_reviewed_pubmed_autonomous_evidence_registration(
            multi_lane,
            multi_lane.prepare(),
            specialty_lane="glioma",
        )


def test_base_refresh_narrows_to_allow_listed_lanes_and_rejects_arbitrary_queries() -> (
    None
):
    calls: list[str] = []
    bundle, report = refresh_neurosurgical_public_literature(
        fetch=_fixture_fetch(calls),
        per_specialty_limit=1,
        specialty_lanes=("chiari_malformation",),
        retrieved_at=_RETRIEVED_AT,
    )
    assert len(calls) == 3
    assert report.specialty_counts == {"chiari_malformation": 1}
    assert bundle["sources"][0]["source_id"] == "pubmed_chiari_malformation"

    for lanes in ((), ("glioma", "glioma"), ("arbitrary query",)):
        with pytest.raises(PublicLiteratureRefreshError):
            refresh_neurosurgical_public_literature(
                fetch=_fixture_fetch([]),
                per_specialty_limit=1,
                specialty_lanes=lanes,
            )
