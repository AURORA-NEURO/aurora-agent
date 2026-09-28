from __future__ import annotations

import json
from urllib.parse import parse_qs, urlsplit

import pytest

import prism_sdk
from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.reviewed_cbioportal_study_retrieval import (
    BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_CBIOPORTAL_TRANSPORT_ID,
    REVIEWED_CBIOPORTAL_STUDIES,
    ReviewedCBioPortalRetrievalAdapter,
    ReviewedCBioPortalRetrievalConfig,
    ReviewedCBioPortalRetrievalError,
    ReviewedCBioPortalRetrievalPlan,
    create_reviewed_cbioportal_autonomous_evidence_registration,
    create_reviewed_cbioportal_execution_metadata,
)


_TRANSPORT = {"transport_id": "fixture.cbioportal", "transport_version": "1", "transport_config_digest": "a" * 64}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"


def _study(study_id: str = "gbm_tcga", *, sample_count: int | None = 619) -> dict[str, object]:
    return {
        "studyId": study_id,
        "cancerTypeId": "gbm" if study_id == "gbm_tcga" else "lgg",
        "name": f"{study_id} public fixture",
        "description": "Aggregate catalogue metadata only.",
        "publicStudy": True,
        "pmid": "22824167",
        "allSampleCount": sample_count,
        "referenceGenome": "hg19",
        "importDate": "2023-01-02",
        "status": 0,
        "clinicalData": [{"unrequested": "must not be retained"}],
    }


def _profiles(study_id: str = "gbm_tcga") -> list[dict[str, object]]:
    return [
        {
            "molecularProfileId": f"{study_id}_mutations",
            "studyId": study_id,
            "molecularAlterationType": "MUTATION_EXTENDED",
            "genericAssayType": None,
            "datatype": "MAF",
            "name": "Mutations",
            "description": "Mutation profile catalogue entry.",
            "showProfileInAnalysisTab": True,
            "patientLevel": False,
            "pivotThreshold": 0.5,
            "molecularData": [{"unrequested": "must not be retained"}],
        },
        {
            "molecularProfileId": f"{study_id}_mrna",
            "studyId": study_id,
            "molecularAlterationType": "MRNA_EXPRESSION",
            "genericAssayType": None,
            "datatype": "CONTINUOUS",
            "name": "mRNA expression",
            "description": None,
            "showProfileInAnalysisTab": True,
            "patientLevel": False,
        },
    ]


def _adapter(responses: list[object], *, study_ids: tuple[str, ...] = ("gbm_tcga",)):
    calls: list[str] = []

    def fetch(url: str) -> object:
        calls.append(url)
        return responses[len(calls) - 1]

    config = ReviewedCBioPortalRetrievalConfig(study_ids=study_ids, **_TRANSPORT)
    return config, ReviewedCBioPortalRetrievalAdapter(config, fetch=fetch), calls


def test_config_plan_and_catalogue_are_fixed_and_exported() -> None:
    assert prism_sdk.ReviewedCBioPortalRetrievalAdapter is ReviewedCBioPortalRetrievalAdapter
    assert dict(REVIEWED_CBIOPORTAL_STUDIES) == {"gbm": "gbm_tcga", "lgg": "lgg_tcga"}
    with pytest.raises(TypeError):
        REVIEWED_CBIOPORTAL_STUDIES["other"] = "other_tcga"  # type: ignore[index]
    builtin = ReviewedCBioPortalRetrievalConfig()
    assert builtin.study_ids == ("gbm_tcga",)
    assert builtin.transport_id == BUILTIN_CBIOPORTAL_TRANSPORT_ID
    assert len(BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST) == 64
    fixture = ReviewedCBioPortalRetrievalConfig(**_TRANSPORT)
    plan = ReviewedCBioPortalRetrievalPlan.create(fixture)
    assert fixture.request_limit == plan.to_dict()["request_limit"] == 2
    assert ReviewedCBioPortalRetrievalConfig.from_dict(fixture.to_dict()) == fixture
    assert ReviewedCBioPortalRetrievalPlan.from_dict(plan.to_dict()).to_dict() == plan.to_dict()
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="unsupported or duplicate"):
        ReviewedCBioPortalRetrievalConfig(study_ids=("other_tcga",), **_TRANSPORT)
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="unsupported or duplicate"):
        ReviewedCBioPortalRetrievalConfig(study_ids=("gbm_tcga", "gbm_tcga"), **_TRANSPORT)
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="distinct reviewed identity"):
        ReviewedCBioPortalRetrievalAdapter(builtin, fetch=lambda _url: {})


def test_approval_dispatches_only_fixed_public_study_and_profile_metadata() -> None:
    config, adapter, calls = _adapter([_study(), _profiles()])
    plan = adapter.prepare()
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert config.request_limit == 2
    assert len(calls) == 2
    first = urlsplit(calls[0])
    second = urlsplit(calls[1])
    assert first.scheme == second.scheme == "https"
    assert first.netloc == second.netloc == "www.cbioportal.org"
    assert first.path == "/api/studies/gbm_tcga" and first.query == ""
    assert second.path == "/api/studies/gbm_tcga/molecular-profiles"
    assert parse_qs(second.query) == {"projection": ["SUMMARY"], "pageSize": ["128"], "pageNumber": ["0"], "sortBy": ["molecularProfileId"], "direction": ["ASC"]}
    bundle = result.to_transient_dict()["bundle"]
    assert bundle["studies"][0]["sample_count"] == 619
    assert [profile["profile_id"] for profile in bundle["molecular_profiles"]] == ["gbm_tcga_mrna", "gbm_tcga_mutations"]
    assert result.receipt["request_count"] == 2
    serialized = json.dumps(bundle).lower()
    assert "case_id" not in serialized and "sample_id" not in serialized and "patient_id" not in serialized
    assert "pivotthreshold" not in serialized
    assert "clinicaldata" not in serialized and "moleculardata" not in serialized


def test_two_studies_are_bounded_and_missing_counts_remain_unknown() -> None:
    _config, adapter, calls = _adapter(
        [_study(sample_count=None), _profiles(), _study("lgg_tcga", sample_count=530), _profiles("lgg_tcga")],
        study_ids=("lgg_tcga", "gbm_tcga"),
    )
    assert adapter.config.study_ids == ("gbm_tcga", "lgg_tcga")
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert [urlsplit(url).path for url in calls] == [
        "/api/studies/gbm_tcga", "/api/studies/gbm_tcga/molecular-profiles",
        "/api/studies/lgg_tcga", "/api/studies/lgg_tcga/molecular-profiles",
    ]
    assert result.receipt["request_count"] == 4
    assert result.receipt["completeness"] == "unknown"
    assert result.bundle["studies"][0]["sample_count"] is None


def test_private_or_mismatched_studies_and_unsafe_or_truncated_profiles_fail_closed() -> None:
    private = _study()
    private["publicStudy"] = False
    _, adapter, calls = _adapter([private, _profiles()])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="public status"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    assert len(calls) == 1
    _, adapter, calls = _adapter([_study("lgg_tcga"), _profiles()])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="identity or public status"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    assert len(calls) == 1
    mismatched = _profiles()
    mismatched[0]["studyId"] = "lgg_tcga"
    _, adapter, calls = _adapter([_study(), mismatched])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="mismatched study identity"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    assert len(calls) == 2
    duplicates = _profiles()
    duplicates[1]["molecularProfileId"] = duplicates[0]["molecularProfileId"]
    _, adapter, _ = _adapter([_study(), duplicates])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="duplicate IDs"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    unsupported = _profiles()
    unsupported[0]["molecularAlterationType"] = "UNREVIEWED_TYPE"
    _, adapter, _ = _adapter([_study(), unsupported])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="unsupported alteration type"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    invalid_id = _profiles()
    invalid_id[0]["molecularProfileId"] = "gbm_tcga_😀"
    _, adapter, _ = _adapter([_study(), invalid_id])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="identifier bound"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    duplicate = '{"studyId":"gbm_tcga","studyId":"gbm_tcga","name":"duplicate field"}'
    _, adapter, _ = _adapter([duplicate, _profiles()])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="duplicate JSON fields"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)
    oversized = [{**_profiles()[0], "molecularProfileId": f"gbm_tcga_{index}"} for index in range(128)]
    _, adapter, _ = _adapter([_study(), oversized])
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="catalogue|single reviewed page"):
        adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_evidence_registration_validates_receipt_and_projects_only_digests() -> None:
    _, adapter, _ = _adapter([_study(), _profiles()])
    plan = adapter.prepare()
    registration = create_reviewed_cbioportal_autonomous_evidence_registration(adapter, plan, study_id="gbm_tcga")
    registry = AutonomousEvidenceAdapterRegistry([registration])
    assert registry.manifests()[0].adapter_id == "reviewed.cbioportal.gbm"
    metadata = create_reviewed_cbioportal_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    request = {"source_id": "cbioportal_gbm", "source_digest": plan.plan_digest, "metadata": metadata}
    transient = registration.acquire({"request": request})
    observations = registration.project(transient, {"requirement": {"label": "cBioPortal catalogue"}})
    assert observations[0]["value_digest"] == transient["receipt"]["bundle_digest"]
    assert "gbm_tcga public fixture" not in json.dumps(observations)
    tampered = json.loads(json.dumps(transient))
    tampered["bundle"]["molecular_profiles"][0]["datatype"] = "CHANGED"
    tampered["bundle"]["bundle_digest"] = content_digest({key: value for key, value in tampered["bundle"].items() if key != "bundle_digest"})
    tampered["receipt"]["bundle_digest"] = tampered["bundle"]["bundle_digest"]
    tampered["receipt"]["receipt_digest"] = content_digest({key: value for key, value in tampered["receipt"].items() if key != "receipt_digest"})
    with pytest.raises(ReviewedCBioPortalRetrievalError, match="profiles|source metadata"):
        registration.project(tampered, {"requirement": {"label": "cBioPortal catalogue"}})
