from __future__ import annotations

import json
from pathlib import Path

import pytest

from prism_sdk.real_data_refresh import (
    RealDataRefreshError,
    atomic_refresh_real_glioma_data,
    bundle_digest,
    refresh_real_glioma_data,
    source_hash,
    validate_real_glioma_bundle,
)


def _fake_fetch(url: str):
    if "clinicaltrials.gov/api/v2/studies" in url:
        return {
            "studies": [{
                "protocolSection": {
                    "identificationModule": {"nctId": "NCT00000001", "briefTitle": "Observed glioma registry study"},
                    "statusModule": {"overallStatus": "RECRUITING", "lastUpdatePostDateStruct": {"date": "2026-01-02"}},
                    "designModule": {
                        "studyType": "INTERVENTIONAL",
                        "phases": ["PHASE1"],
                        "enrollmentInfo": {"count": 12},
                    },
                    "armsInterventionsModule": {"interventions": [{"name": "Biomarker analysis"}]},
                }
            }],
        }
    if "api.gdc.cancer.gov/projects/TCGA-GBM" in url:
        return {"data": {"project_id": "TCGA-GBM", "name": "Glioblastoma Multiforme", "primary_site": ["Brain"], "disease_type": ["Glioblastoma"]}}
    if "api.gdc.cancer.gov/cases" in url:
        return {"data": {"pagination": {"total": 617}}}
    if "api.gdc.cancer.gov/files" in url:
        return {"data": {"aggregations": {"data_type": {"buckets": [{"key": "Gene Expression Quantification", "doc_count": 42}]}}}}
    if url.endswith("/api/studies?keyword=gbm"):
        return [{"studyId": "gbm_test", "name": "Observed GBM study", "description": "Public aggregate study metadata", "publicStudy": True, "pmid": "12345678"}]
    if url.endswith("/api/studies/gbm_test"):
        return {"studyId": "gbm_test", "allSampleCount": 2}
    if "/api/studies/gbm_test/molecular-profiles" in url:
        return [{
            "studyId": "gbm_test",
            "molecularProfileId": "gbm_test_mutations",
            "name": "Observed mutations",
            "molecularAlterationType": "MUTATION_EXTENDED",
            "datatype": "MAF",
            "description": "Aggregate assay metadata",
            "showProfileInAnalysisTab": True,
            "patientLevel": True,
        }]
    if "esearch.fcgi" in url:
        return {"esearchresult": {"idlist": ["12345678"]}}
    if "esummary.fcgi" in url:
        return {"result": {"12345678": {
            "title": "Observed glioma molecular cohort",
            "fulljournalname": "Journal of Neurosurgery",
            "pubdate": "2026 Jan 02",
            "articleids": [{"idtype": "doi", "value": "10.1000/aurora"}],
        }}}
    if "efetch.fcgi" in url:
        return (
            "<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>12345678</PMID>"
            "<Article><Abstract><AbstractText>Observed source abstract</AbstractText></Abstract>"
            "<PublicationTypeList><PublicationType>Journal Article</PublicationType></PublicationTypeList>"
            "</Article><MeshHeadingList><MeshHeading><DescriptorName>Glioma</DescriptorName>"
            "</MeshHeading></MeshHeadingList></MedlineCitation></PubmedArticle></PubmedArticleSet>"
        ).encode()
    raise AssertionError(f"unexpected URL: {url}")


def _refresh_kwargs() -> dict[str, object]:
    return {
        "fetch": _fake_fetch,
        "gdc_project_ids": ("TCGA-GBM",),
        "trial_page_size": 1,
        "portal_study_ids": ("gbm_test",),
        "portal_study_limit": 1,
        "pubmed_limit": 1,
        "retrieved_at": "2026-08-30T06:02:51Z",
    }


def test_refresh_builds_real_bundle_with_rust_compatible_digests() -> None:
    bundle, report = refresh_real_glioma_data(**_refresh_kwargs())
    validate_real_glioma_bundle(bundle)
    assert report.bundle_digest == bundle_digest(bundle)
    assert report.source_count == 5
    assert report.record_count == 6
    assert report.genomic_project_count == 1
    assert report.molecular_profile_count == 1
    assert report.reference_count == 1
    assert bundle["synthetic_data"] is False
    assert all(len(source["content_sha256"]) == 64 for source in bundle["sources"])


def test_checked_in_snapshot_replays_with_python_digest_contract() -> None:
    snapshot = Path(__file__).parents[2] / "data" / "neurosurgery" / "glioma_public_snapshot.json"
    bundle = json.loads(snapshot.read_text(encoding="utf-8"))
    validate_real_glioma_bundle(bundle)
    source_digests = {source["source_id"]: source["content_sha256"] for source in bundle["sources"]}
    assert {source_id: source_hash(bundle, source_id) for source_id in source_digests} == source_digests
    assert bundle_digest(bundle) == "16ed80e14703f11ff3b408d73eb6c159045777dacc249a659c6f112ac8b477cb"


def test_refresh_rejects_missing_public_facets_or_synthetic_text() -> None:
    def missing_facets(url: str):
        response = _fake_fetch(url)
        if "api.gdc.cancer.gov/files" in url:
            return {"data": {"aggregations": {"data_type": {"buckets": []}}}}
        return response

    with pytest.raises(RealDataRefreshError):
        refresh_real_glioma_data(**{**_refresh_kwargs(), "fetch": missing_facets})

    bundle, _ = refresh_real_glioma_data(**_refresh_kwargs())
    bundle["references"][0]["title"] = "Synthetic fixture guideline"
    with pytest.raises(RealDataRefreshError):
        validate_real_glioma_bundle(bundle)


def test_partial_pubmed_dates_remain_missing_instead_of_becoming_january_first() -> None:
    def partial_date_fetch(url: str):
        response = _fake_fetch(url)
        if "esummary.fcgi" in url:
            response = dict(response)
            response["result"] = dict(response["result"])
            article = dict(response["result"]["12345678"])
            article["pubdate"] = "2026 Jan"
            response["result"]["12345678"] = article
        return response

    bundle, _ = refresh_real_glioma_data(**{**_refresh_kwargs(), "fetch": partial_date_fetch})
    assert bundle["literature"][0]["publication_date"] is None

    def year_only_fetch(url: str):
        response = _fake_fetch(url)
        if "esummary.fcgi" in url:
            response = dict(response)
            response["result"] = dict(response["result"])
            article = dict(response["result"]["12345678"])
            article["pubdate"] = "2026"
            response["result"]["12345678"] = article
        return response

    bundle, _ = refresh_real_glioma_data(**{**_refresh_kwargs(), "fetch": year_only_fetch})
    assert bundle["literature"][0]["publication_date"] is None


def test_atomic_refresh_replaces_only_after_candidate_validation(tmp_path: Path) -> None:
    output = tmp_path / "glioma.json"
    report = atomic_refresh_real_glioma_data(output, **_refresh_kwargs())
    persisted = json.loads(output.read_text(encoding="utf-8"))
    assert report.output_path == str(output)
    assert bundle_digest(persisted) == report.bundle_digest
    assert not list(tmp_path.glob("*.candidate"))

    output.write_text("last-known-good", encoding="utf-8")

    def failed_fetch(_url: str):
        raise OSError("transport unavailable")

    with pytest.raises(RealDataRefreshError):
        atomic_refresh_real_glioma_data(output, **{**_refresh_kwargs(), "fetch": failed_fetch})
    assert output.read_text(encoding="utf-8") == "last-known-good"
