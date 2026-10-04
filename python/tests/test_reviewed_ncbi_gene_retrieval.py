from __future__ import annotations

import json
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from urllib.parse import parse_qs, urlsplit

import pytest

import prism_sdk
from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.reviewed_ncbi_gene_retrieval import (
    BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_NCBI_GENE_TRANSPORT_ID,
    REVIEWED_NCBI_GENE_CATALOGUE,
    ReviewedNcbiGeneRetrievalAdapter,
    ReviewedNcbiGeneRetrievalConfig,
    ReviewedNcbiGeneRetrievalError,
    ReviewedNcbiGeneRetrievalPlan,
    create_reviewed_ncbi_gene_autonomous_evidence_registration,
    create_reviewed_ncbi_gene_execution_metadata,
)
from prism_sdk.reviewed_pubmed_retrieval import (
    ReviewedPubMedRetrievalAdapter,
    ReviewedPubMedRetrievalConfig,
)


_TRANSPORT = {
    "transport_id": "fixture.ncbi-gene",
    "transport_version": "1",
    "transport_config_digest": "a" * 64,
}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"
_SYMBOLS = ("IDH1", "MGMT")


def _summary() -> dict[str, object]:
    return {
        "header": {"type": "esummary", "version": "0.3"},
        "result": {
            "uids": ["3417", "4255"],
            "3417": {
                "uid": "3417", "name": "IDH1", "description": "isocitrate dehydrogenase 1",
                "chromosome": "2", "maplocation": "2q34", "otheraliases": "HEL-216, IDP",
                "organism": {"taxid": 9606, "scientificname": "Homo sapiens"},
                "summary": "This full functional summary must remain transient and unprojected.",
                "genomicinfo": [{"chraccver": "NC_000002.12", "chrstart": 1}],
            },
            "4255": {
                "uid": "4255", "name": "MGMT", "description": "O-6-methylguanine-DNA methyltransferase",
                "chromosome": "10", "maplocation": "10q26.3", "otheraliases": "AGT, MGMT1",
                "organism": {"taxid": 9606, "scientificname": "Homo sapiens"},
                "summary": "Another non-projected functional summary.",
            },
        },
    }


def _adapter(response: object | None = None, *, symbols: tuple[str, ...] = _SYMBOLS, **config_options: object):
    calls: list[str] = []

    def fetch(url: str) -> object:
        calls.append(url)
        return response if response is not None else _summary()

    config = ReviewedNcbiGeneRetrievalConfig(gene_symbols=symbols, **_TRANSPORT, **config_options)
    return config, ReviewedNcbiGeneRetrievalAdapter(config, fetch=fetch), calls


def test_config_plan_catalogue_and_public_exports_are_fixed_and_digest_bound() -> None:
    assert prism_sdk.ReviewedNcbiGeneRetrievalAdapter is ReviewedNcbiGeneRetrievalAdapter
    assert dict(REVIEWED_NCBI_GENE_CATALOGUE) == {
        "IDH1": "3417", "IDH2": "3418", "MGMT": "4255", "EGFR": "1956",
        "TERT": "7015", "TP53": "7157", "ATRX": "546", "NF1": "4763",
        "PTEN": "5728", "CDKN2A": "1029", "PDGFRA": "5156", "BRAF": "673",
    }
    with pytest.raises(TypeError):
        REVIEWED_NCBI_GENE_CATALOGUE["OTHER"] = "1"  # type: ignore[index]
    builtin = ReviewedNcbiGeneRetrievalConfig()
    assert builtin.transport_id == BUILTIN_NCBI_GENE_TRANSPORT_ID
    assert builtin.transport_config_digest == BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST
    config = ReviewedNcbiGeneRetrievalConfig(gene_symbols=("MGMT", "IDH1"), **_TRANSPORT)
    assert config.gene_symbols == _SYMBOLS
    plan = ReviewedNcbiGeneRetrievalPlan.create(config)
    assert plan.to_dict()["request_limit"] == 1
    assert ReviewedNcbiGeneRetrievalConfig.from_dict(config.to_dict()) == config
    assert ReviewedNcbiGeneRetrievalPlan.from_dict(plan.to_dict()).to_dict() == plan.to_dict()
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="unsupported or duplicate"):
        ReviewedNcbiGeneRetrievalConfig(gene_symbols=("OTHER",), **_TRANSPORT)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="unsupported or duplicate"):
        ReviewedNcbiGeneRetrievalConfig(gene_symbols=("IDH1", "IDH1"), **_TRANSPORT)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="invalid shape"):
        ReviewedNcbiGeneRetrievalConfig.from_dict({**config.to_dict(), "query": "arbitrary"})
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="distinct reviewed identity"):
        ReviewedNcbiGeneRetrievalAdapter(builtin, fetch=lambda _url: {})


def test_explicit_approval_pins_one_esummary_request_and_projects_only_fixed_human_metadata() -> None:
    response_text = json.dumps(_summary(), separators=(",", ":"), ensure_ascii=False)
    _config, adapter, calls = _adapter(response_text)
    plan = adapter.prepare()
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert result.receipt["bundle_digest"] == "9943c5606ff60d2612c3e9d040b3491d3e7affaeb56e6b1a6becdac042d9102a"
    assert result.receipt["receipt_digest"] == "980bafb3726e18411fb6625093b35e186bd062445a1450be6e459b556bbc9435"
    assert len(calls) == 1
    parsed = urlsplit(calls[0])
    assert parsed.scheme == "https" and parsed.netloc == "eutils.ncbi.nlm.nih.gov"
    assert parsed.path == "/entrez/eutils/esummary.fcgi"
    assert parse_qs(parsed.query) == {"db": ["gene"], "id": ["3417,4255"], "retmode": ["json"]}
    assert result.receipt["request_count"] == 1
    assert result.receipt["response_bytes"] == len(response_text.encode("utf-8"))
    assert result.receipt["catalogue_coverage"] == "complete"
    assert [gene["symbol"] for gene in result.bundle["genes"]] == ["IDH1", "MGMT"]
    assert result.bundle["genes"][0] == {
        "source_id": "ncbi_gene", "gene_id": "3417", "symbol": "IDH1",
        "description": "isocitrate dehydrogenase 1", "chromosome": "2",
        "map_location": "2q34", "aliases": ["HEL-216", "IDP"], "organism_taxid": 9606,
    }
    serialized = json.dumps(result.to_transient_dict()["bundle"]).lower()
    assert "full functional summary" not in serialized
    assert "genomicinfo" not in serialized
    assert "chrstart" not in serialized
    assert result.to_dict()["retention"] == "metadata_only"


def test_registered_ncbi_identity_is_paired_digest_bound_and_never_serialized_as_contact_text() -> None:
    email = "researcher@example.org"
    config, adapter, calls = _adapter(ncbi_tool="aurora_agent", ncbi_email=email)
    serialized_config = json.dumps(config.to_dict())
    assert email not in repr(config)
    assert "aurora_agent" not in repr(config)
    assert email not in serialized_config
    assert "aurora_agent" not in serialized_config
    plan = adapter.prepare()
    assert email not in json.dumps(plan.to_dict())
    assert "aurora_agent" not in json.dumps(plan.to_dict())
    metadata = create_reviewed_ncbi_gene_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert email not in json.dumps(metadata) and "aurora_agent" not in json.dumps(metadata)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="provided together"):
        ReviewedNcbiGeneRetrievalConfig(ncbi_tool="aurora_agent", **_TRANSPORT)
    restored = ReviewedNcbiGeneRetrievalConfig.from_dict(config.to_dict(), ncbi_tool="aurora_agent", ncbi_email=email)
    assert restored == config
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="registration identity changed"):
        ReviewedNcbiGeneRetrievalConfig.from_dict(config.to_dict(), ncbi_tool="different_tool", ncbi_email=email)
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    query = parse_qs(urlsplit(calls[0]).query)
    assert query["tool"] == ["aurora_agent"] and query["email"] == [email]
    assert email not in json.dumps(result.to_transient_dict())
    assert "aurora_agent" not in json.dumps(result.to_transient_dict())
    assert email not in result.bundle["sources"][0]["uri"]


def test_wrong_taxid_symbol_uid_coverage_and_duplicate_source_fields_fail_closed() -> None:
    wrong_taxid = _summary()
    wrong_taxid["result"]["3417"]["organism"]["taxid"] = 10090  # type: ignore[index]
    _, adapter, _ = _adapter(wrong_taxid)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="human organism"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    wrong_symbol = _summary()
    wrong_symbol["result"]["3417"]["name"] = "IDH2"  # type: ignore[index]
    _, adapter, _ = _adapter(wrong_symbol)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="symbol differs"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    incomplete = _summary()
    incomplete["result"]["uids"] = ["3417"]  # type: ignore[index]
    _, adapter, _ = _adapter(incomplete)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="incomplete or reordered"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    duplicate = ReviewedNcbiGeneRetrievalAdapter(
        ReviewedNcbiGeneRetrievalConfig(gene_symbols=_SYMBOLS, **_TRANSPORT),
        fetch=lambda _url: '{"result":{"uids":["3417","4255"],"3417":{"uid":"3417","uid":"4255"}}}',
    )
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="duplicate JSON fields"):
        duplicate.execute(duplicate.prepare(), approve_source_dispatch=True)


def test_oversized_and_deep_gene_responses_fail_closed() -> None:
    _, adapter, _ = _adapter(" " * 1_000_001)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="byte bound"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    too_deep = _summary()
    child: dict[str, object] = {}
    cursor = child
    for _ in range(40):
        nested: dict[str, object] = {}
        cursor["nested"] = nested
        cursor = nested
    too_deep["unrequested"] = child
    _, adapter, _ = _adapter(too_deep)
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="structural bound"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_evidence_registration_validates_review_and_projects_only_digest_provenance() -> None:
    _, adapter, _ = _adapter()
    plan = adapter.prepare()
    registration = create_reviewed_ncbi_gene_autonomous_evidence_registration(adapter, plan)
    assert AutonomousEvidenceAdapterRegistry([registration]).manifests()[0].adapter_id == "reviewed.ncbi_gene"
    metadata = create_reviewed_ncbi_gene_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    request = {"source_id": "ncbi_gene", "source_digest": plan.plan_digest, "metadata": metadata}
    transient = registration.acquire({"request": request})
    observations = registration.project(transient, {"requirement": {"label": "NCBI Gene catalogue"}})
    assert observations[0]["kind"] == "provenance"
    assert observations[0]["value_digest"] == transient["receipt"]["bundle_digest"]
    assert "isocitrate dehydrogenase" not in json.dumps(observations)
    forged = {**metadata, "approve_source_dispatch": False}
    forged["metadata_digest"] = content_digest({key: value for key, value in forged.items() if key != "metadata_digest"})
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="failed review binding"):
        registration.acquire({"request": {**request, "metadata": forged}})
    tampered = json.loads(json.dumps(transient))
    tampered["bundle"]["genes"][0]["gene_id"] = "4255"
    tampered["bundle"]["bundle_digest"] = content_digest({key: value for key, value in tampered["bundle"].items() if key != "bundle_digest"})
    tampered["receipt"]["bundle_digest"] = tampered["bundle"]["bundle_digest"]
    tampered["receipt"]["receipt_digest"] = content_digest({key: value for key, value in tampered["receipt"].items() if key != "receipt_digest"})
    with pytest.raises(ReviewedNcbiGeneRetrievalError, match="reviewed catalogue"):
        registration.project(tampered, {"requirement": {"label": "NCBI Gene catalogue"}})


def test_gene_and_pubmed_dispatches_share_one_process_rate_limiter() -> None:
    dispatch_times: list[float] = []
    dispatch_lock = threading.Lock()

    def record_dispatch() -> None:
        with dispatch_lock:
            dispatch_times.append(time.monotonic())

    def gene_fetch(_url: str) -> object:
        record_dispatch()
        return _summary()

    def pubmed_fetch(url: str) -> object:
        record_dispatch()
        endpoint = urlsplit(url).path.rsplit("/", 1)[-1]
        if endpoint == "esearch.fcgi":
            return {"esearchresult": {"idlist": ["20000"]}}
        if endpoint == "esummary.fcgi":
            return {"result": {"20000": {"title": "Reviewed study", "fulljournalname": "Journal", "pubdate": "2025 Jan 02"}}}
        return (
            '<?xml version="1.0" ?>'
            '<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>20000</PMID>'
            '<Article><Abstract><AbstractText>Reviewed abstract</AbstractText></Abstract>'
            '<PublicationTypeList><PublicationType>Journal Article</PublicationType>'
            '</PublicationTypeList></Article></MedlineCitation></PubmedArticle></PubmedArticleSet>'
        ).encode()

    gene_config = ReviewedNcbiGeneRetrievalConfig(gene_symbols=_SYMBOLS, **_TRANSPORT)
    gene_adapter = ReviewedNcbiGeneRetrievalAdapter(gene_config, fetch=gene_fetch)
    pubmed_config = ReviewedPubMedRetrievalConfig(
        specialty_lanes=("glioma",),
        per_specialty_limit=1,
        transport_id="fixture.shared-ncbi",
        transport_version="1",
        transport_config_digest=content_digest({"fixture": "shared-ncbi-rate-limit"}),
    )
    pubmed_adapter = ReviewedPubMedRetrievalAdapter(pubmed_config, fetch=pubmed_fetch)

    with ThreadPoolExecutor(max_workers=2) as workers:
        gene = workers.submit(
            gene_adapter.execute,
            gene_adapter.prepare(),
            approve_source_dispatch=True,
            retrieved_at=_RETRIEVED_AT,
        )
        pubmed = workers.submit(
            pubmed_adapter.execute,
            pubmed_adapter.prepare(),
            approve_source_dispatch=True,
            retrieved_at=_RETRIEVED_AT,
        )
        gene.result()
        pubmed.result()

    ordered = sorted(dispatch_times)
    assert len(ordered) == 4
    assert all(right - left >= 0.30 for left, right in zip(ordered, ordered[1:]))
