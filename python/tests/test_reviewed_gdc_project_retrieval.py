from __future__ import annotations

import json
from urllib.parse import parse_qs, urlsplit

import pytest

import prism_sdk
from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.reviewed_gdc_project_retrieval import (
    BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_GDC_TRANSPORT_ID,
    REVIEWED_GDC_PROJECTS,
    ReviewedGdcRetrievalAdapter,
    ReviewedGdcRetrievalConfig,
    ReviewedGdcRetrievalError,
    ReviewedGdcRetrievalPlan,
    create_reviewed_gdc_autonomous_evidence_registration,
    create_reviewed_gdc_execution_metadata,
)


_TRANSPORT = {
    "transport_id": "fixture.nci-gdc",
    "transport_version": "1",
    "transport_config_digest": "a" * 64,
}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"


def _project(project_id: str = "TCGA-GBM", *, missing_counts: bool = False) -> dict[str, object]:
    summary: dict[str, object] = {
        "data_categories": [
            {"data_category": "Transcriptome Profiling", "case_count": 610, "file_count": 1_200},
            {"data_category": "Clinical", "case_count": 615, "file_count": 5},
        ],
    }
    if not missing_counts:
        summary.update({"case_count": 615, "file_count": 2_405})
    return {
        "data": {
            "project_id": project_id,
            "name": f"{project_id} aggregate metadata fixture",
            "disease_type": ["Gliomas", "Central Nervous System Tumors"],
            "primary_site": ["Brain"],
            "state": "open",
            "released": True,
            "summary": summary,
        },
        "warnings": {},
    }


def _adapter(responses: list[object], *, projects: tuple[str, ...] = ("TCGA-GBM",)):
    calls: list[str] = []

    def fetch(url: str) -> object:
        calls.append(url)
        return responses[len(calls) - 1]

    config = ReviewedGdcRetrievalConfig(project_ids=projects, **_TRANSPORT)
    return config, ReviewedGdcRetrievalAdapter(config, fetch=fetch), calls


def test_config_plan_and_catalogue_are_fixed_and_digest_bound() -> None:
    assert prism_sdk.ReviewedGdcRetrievalAdapter is ReviewedGdcRetrievalAdapter
    assert dict(REVIEWED_GDC_PROJECTS) == {"gbm": "TCGA-GBM", "lgg": "TCGA-LGG"}
    with pytest.raises(TypeError):
        REVIEWED_GDC_PROJECTS["other"] = "TCGA-OTHER"  # type: ignore[index]
    builtin = ReviewedGdcRetrievalConfig()
    assert builtin.transport_id == BUILTIN_GDC_TRANSPORT_ID
    assert builtin.transport_config_digest == BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST
    config = ReviewedGdcRetrievalConfig(**_TRANSPORT)
    plan = ReviewedGdcRetrievalPlan.create(config)
    assert ReviewedGdcRetrievalConfig.from_dict(config.to_dict()) == config
    assert ReviewedGdcRetrievalPlan.from_dict(plan.to_dict()).to_dict() == plan.to_dict()
    assert plan.to_dict()["request_limit"] == 1
    with pytest.raises(ReviewedGdcRetrievalError, match="unsupported or duplicate"):
        ReviewedGdcRetrievalConfig(project_ids=("TCGA-OTHER",), **_TRANSPORT)
    with pytest.raises(ReviewedGdcRetrievalError, match="unsupported or duplicate"):
        ReviewedGdcRetrievalConfig(project_ids=("TCGA-GBM", "TCGA-GBM"), **_TRANSPORT)
    with pytest.raises(ReviewedGdcRetrievalError, match="invalid shape"):
        ReviewedGdcRetrievalConfig.from_dict({**config.to_dict(), "arbitrary_query": "TCGA-GBM"})
    with pytest.raises(ReviewedGdcRetrievalError, match="distinct reviewed identity"):
        ReviewedGdcRetrievalAdapter(builtin, fetch=lambda _url: {})


def test_explicit_approval_pins_gdc_project_request_and_normalizes_aggregate_metadata() -> None:
    response = _project()
    response_text = json.dumps(response, separators=(",", ":"), ensure_ascii=False)
    config, adapter, calls = _adapter([response_text])
    plan = adapter.prepare()
    with pytest.raises(ReviewedGdcRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert len(calls) == 1
    parsed = urlsplit(calls[0])
    assert parsed.scheme == "https" and parsed.netloc == "api.gdc.cancer.gov"
    assert parsed.path == "/projects/TCGA-GBM"
    assert parse_qs(parsed.query) == {
        "format": ["json"],
        "expand": ["summary,summary.data_categories"],
        "fields": ["project_id,name,disease_type,primary_site,state,released,summary.case_count,summary.file_count,summary.data_categories"],
    }
    assert result.receipt["response_bytes"] == len(response_text.encode("utf-8"))
    assert result.receipt["request_count"] == 1
    assert result.receipt["completeness"] == "complete"
    project = result.bundle["projects"][0]
    assert project["case_count"] == 615
    assert project["file_count"] == 2_405
    assert project["data_categories"] == [
        {"data_category": "Clinical", "case_count": 615, "file_count": 5},
        {"data_category": "Transcriptome Profiling", "case_count": 610, "file_count": 1_200},
    ]
    assert "cases" not in json.dumps(result.bundle).lower()
    assert "sample_id" not in json.dumps(result.bundle).lower()
    assert "file_id" not in json.dumps(result.bundle).lower()
    assert config.project_ids == ("TCGA-GBM",)


def test_missing_aggregate_counts_stay_unknown_and_two_project_plan_is_bounded() -> None:
    config, adapter, calls = _adapter(
        [_project("TCGA-GBM", missing_counts=True), _project("TCGA-LGG")],
        projects=("TCGA-LGG", "TCGA-GBM"),
    )
    assert config.project_ids == ("TCGA-GBM", "TCGA-LGG")
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert [urlsplit(url).path for url in calls] == ["/projects/TCGA-GBM", "/projects/TCGA-LGG"]
    assert result.receipt["request_count"] == 2
    assert result.receipt["completeness"] == "unknown"
    assert result.bundle["projects"][0]["case_count"] is None
    assert result.bundle["projects"][0]["file_count"] is None
    assert result.bundle["projects"][1]["case_count"] == 615


def test_duplicate_fields_foreign_projects_and_malformed_counts_fail_closed() -> None:
    config = ReviewedGdcRetrievalConfig(**_TRANSPORT)
    duplicate = ReviewedGdcRetrievalAdapter(config, fetch=lambda _url: '{"data":{"project_id":"TCGA-GBM","project_id":"TCGA-LGG"}}')
    with pytest.raises(ReviewedGdcRetrievalError, match="duplicate JSON fields"):
        duplicate.execute(duplicate.prepare(), approve_source_dispatch=True)
    foreign = ReviewedGdcRetrievalAdapter(config, fetch=lambda _url: _project("TCGA-LGG"))
    with pytest.raises(ReviewedGdcRetrievalError, match="identity does not match"):
        foreign.execute(foreign.prepare(), approve_source_dispatch=True)
    malformed = _project()
    malformed["data"]["summary"]["case_count"] = True  # type: ignore[index]
    invalid = ReviewedGdcRetrievalAdapter(config, fetch=lambda _url: malformed)
    with pytest.raises(ReviewedGdcRetrievalError, match="integer range"):
        invalid.execute(invalid.prepare(), approve_source_dispatch=True)


def test_invalid_unicode_in_gdc_response_fails_as_a_bounded_adapter_error() -> None:
    config = ReviewedGdcRetrievalConfig(**_TRANSPORT)
    for response in ('{"data":"\\ud800"}', '{"data":"\ud800"}'):
        adapter = ReviewedGdcRetrievalAdapter(config, fetch=lambda _url, response=response: response)
        with pytest.raises(ReviewedGdcRetrievalError, match="Unicode|UTF-8"):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True)


def test_evidence_registration_checks_approval_and_projects_only_digests() -> None:
    _, adapter, _calls = _adapter([_project()])
    plan = adapter.prepare()
    registration = create_reviewed_gdc_autonomous_evidence_registration(adapter, plan, project_id="TCGA-GBM")
    registry = AutonomousEvidenceAdapterRegistry([registration])
    assert registry.manifests()[0].adapter_id == "reviewed.gdc.gbm"
    metadata = create_reviewed_gdc_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    request = {"source_id": "gdc_gbm", "source_digest": plan.plan_digest, "metadata": metadata}
    transient = registration.acquire({"request": request})
    observations = registration.project(transient, {"requirement": {"label": "GDC cohort inventory"}})
    assert observations[0]["kind"] == "provenance"
    assert observations[0]["value_digest"] == transient["receipt"]["bundle_digest"]
    assert "aggregate metadata fixture" not in json.dumps(observations)
    forged = {**metadata, "approve_source_dispatch": False}
    forged["metadata_digest"] = content_digest({key: value for key, value in forged.items() if key != "metadata_digest"})
    with pytest.raises(ReviewedGdcRetrievalError, match="failed review binding"):
        registration.acquire({"request": {**request, "metadata": forged}})
    tampered = json.loads(json.dumps(transient))
    tampered["bundle"]["projects"][0]["case_count"] = 0
    bundle_unsigned = {key: value for key, value in tampered["bundle"].items() if key != "bundle_digest"}
    tampered["bundle"]["bundle_digest"] = content_digest(bundle_unsigned)
    tampered["receipt"]["bundle_digest"] = tampered["bundle"]["bundle_digest"]
    receipt_unsigned = {key: value for key, value in tampered["receipt"].items() if key != "receipt_digest"}
    tampered["receipt"]["receipt_digest"] = content_digest(receipt_unsigned)
    with pytest.raises(ReviewedGdcRetrievalError, match="source metadata"):
        registration.project(tampered, {"requirement": {"label": "GDC cohort inventory"}})
