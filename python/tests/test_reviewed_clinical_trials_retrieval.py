from __future__ import annotations

import json
from urllib.parse import parse_qs, urlsplit

import pytest
import prism_sdk

from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.reviewed_clinical_trials_retrieval import (
    BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID,
    MAX_REVIEWED_CLINICAL_TRIALS_RECORDS,
    REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA,
    ReviewedClinicalTrialsRetrievalAdapter,
    ReviewedClinicalTrialsRetrievalConfig,
    ReviewedClinicalTrialsRetrievalError,
    create_reviewed_clinical_trials_autonomous_evidence_registration,
    create_reviewed_clinical_trials_execution_metadata,
)


_TRANSPORT = {
    "transport_id": "fixture.clinicaltrials",
    "transport_version": "1",
    "transport_config_digest": "a" * 64,
}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"


def _study(nct_id: str = "NCT01234567", title: str = "Reviewed registry study") -> dict[str, object]:
    return {
        "protocolSection": {
            "identificationModule": {"nctId": nct_id, "briefTitle": title},
            "statusModule": {
                "overallStatus": "RECRUITING",
                "lastUpdatePostDateStruct": {"date": "2025-01"},
            },
            "designModule": {
                "phases": ["PHASE2"],
                "studyType": "INTERVENTIONAL",
                "enrollmentInfo": {"count": 42},
            },
            "armsInterventionsModule": {
                "interventions": [{"name": "Study drug"}, {"name": None}],
            },
        }
    }


def _response(studies: list[dict[str, object]], total: int, token: str | None = None) -> dict[str, object]:
    payload: dict[str, object] = {"totalCount": total, "studies": studies}
    if token is not None:
        payload["nextPageToken"] = token
    return payload


def _adapter(responses: list[object], *, lanes: tuple[str, ...] = ("glioblastoma",), **options: object):
    calls: list[str] = []

    def fetch(url: str) -> object:
        calls.append(url)
        return responses[len(calls) - 1]

    config = ReviewedClinicalTrialsRetrievalConfig(
        condition_lanes=lanes,
        page_size=options.get("page_size", 2),
        max_pages=options.get("max_pages", 2),
        **_TRANSPORT,
    )
    return config, ReviewedClinicalTrialsRetrievalAdapter(config, fetch=fetch), calls


def test_preflight_is_pure_deterministic_and_restricts_source_scope() -> None:
    assert prism_sdk.ReviewedClinicalTrialsRetrievalAdapter is ReviewedClinicalTrialsRetrievalAdapter
    calls: list[str] = []
    config = ReviewedClinicalTrialsRetrievalConfig(**_TRANSPORT)
    adapter = ReviewedClinicalTrialsRetrievalAdapter(config, fetch=lambda url: calls.append(url))
    first = adapter.prepare()
    assert adapter.prepare().to_dict() == first.to_dict()
    assert calls == []
    assert first.config.to_dict()["fields"] == [
        "NCTId", "BriefTitle", "OverallStatus", "Phase", "LastUpdatePostDate",
        "StudyType", "EnrollmentCount", "InterventionName",
    ]
    assert MAX_REVIEWED_CLINICAL_TRIALS_RECORDS == 1000
    with pytest.raises(ReviewedClinicalTrialsRetrievalError):
        ReviewedClinicalTrialsRetrievalConfig(condition_lanes=("cardiology",))
    with pytest.raises(ReviewedClinicalTrialsRetrievalError):
        ReviewedClinicalTrialsRetrievalConfig(condition_lanes=("__proto__",))
    with pytest.raises(ReviewedClinicalTrialsRetrievalError):
        ReviewedClinicalTrialsRetrievalConfig(condition_lanes=(["glioma"],))  # type: ignore[arg-type]


def test_transport_identity_cannot_be_confused_with_the_builtin_transport() -> None:
    builtin = ReviewedClinicalTrialsRetrievalConfig()
    assert builtin.transport_id == BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID
    assert builtin.transport_config_digest == BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST
    assert builtin.transport_config_digest == "88be25f8d87736595bfef98b41fd73ed7a2c869f043e0c7b903da68c55d62328"
    assert builtin.config_digest == "985018e7076de7e76bef6e4a68e78ba9a699a0f979c234db4ff6da6a149135a1"
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="distinct reviewed identity"):
        ReviewedClinicalTrialsRetrievalAdapter(builtin, fetch=lambda _url: {})
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="identity is not exact"):
        ReviewedClinicalTrialsRetrievalAdapter(ReviewedClinicalTrialsRetrievalConfig(**_TRANSPORT))


def test_execution_requires_literal_approval_and_preserves_transient_only_study_values() -> None:
    _, adapter, calls = _adapter([_response([_study()], 1)])
    plan = adapter.prepare()
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="literal source-dispatch approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []

    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    bundle = result.bundle
    assert bundle["trials"][0]["nct_id"] == "NCT01234567"
    assert bundle["trials"][0]["intervention_names"] == ["Study drug"]
    assert result.receipt["schema"] == REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA
    serialized = json.dumps(result.to_dict())
    assert "Reviewed registry study" not in serialized
    assert "Study drug" not in serialized
    bundle["trials"][0]["title"] = "mutated"
    assert result.bundle["trials"][0]["title"] == "Reviewed registry study"


def test_pagination_is_exact_bounded_and_reports_truncation_and_totals() -> None:
    page_token = "cursor/+=="
    _, adapter, calls = _adapter([
        _response([_study()], 2, page_token),
        _response([_study("NCT76543210", "Second study")], 2),
    ])
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert len(calls) == 2
    for index, url in enumerate(calls):
        parsed = urlsplit(url)
        assert parsed.scheme == "https"
        assert parsed.netloc == "clinicaltrials.gov"
        assert parsed.path == "/api/v2/studies"
        query = parse_qs(parsed.query, strict_parsing=True)
        assert query["format"] == ["json"]
        assert query["pageSize"] == ["2"]
        assert query["query.cond"] == ["Glioblastoma"]
        if index == 0:
            assert "pageToken" not in query
        else:
            assert query["pageToken"] == [page_token]
    assert result.receipt["request_count"] == 2
    assert result.receipt["reported_total_count"] == 2
    assert result.receipt["truncated"] is False
    assert [row["nct_id"] for row in result.bundle["trials"]] == ["NCT01234567", "NCT76543210"]

    _, truncated_adapter, _ = _adapter([_response([_study()], 3)])
    truncated = truncated_adapter.execute(
        truncated_adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT
    )
    assert truncated.receipt["truncated"] is True
    assert truncated.receipt["omitted_record_count"] == 2


def test_response_parser_and_page_integrity_fail_closed() -> None:
    bad_payloads = [
        '{"totalCount":1,"totalCount":1,"studies":[]}',
        '{"totalCount":0,"studies":[],"ignored":1e400}',
        _response([_study("NCT123")], 1),
        _response([_study(), _study()], 1),
        _response([_study()], 0),
        _response([_study()], 1, "same"),
        _response([], 0, "same"),
    ]
    for payload in bad_payloads:
        responses = [payload, payload]
        _, adapter, _ = _adapter(responses)
        with pytest.raises(ReviewedClinicalTrialsRetrievalError):
            adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)

    nested = "null"
    for _ in range(66):
        nested = f"[{nested}]"
    _, deep_adapter, _ = _adapter([f'{{"totalCount":0,"studies":[],"ignored":{nested}}}'])
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="tree bound"):
        deep_adapter.execute(deep_adapter.prepare(), approve_source_dispatch=True)

    invalid = _study()
    invalid["protocolSection"]["statusModule"]["lastUpdatePostDateStruct"]["date"] = "2025-19-44"  # type: ignore[index]
    _, invalid_date_adapter, _ = _adapter([_response([invalid], 1)])
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="calendar date"):
        invalid_date_adapter.execute(invalid_date_adapter.prepare(), approve_source_dispatch=True)

    def sensitive_transport(_url: str) -> object:
        raise RuntimeError("sensitive upstream response detail")

    config = ReviewedClinicalTrialsRetrievalConfig(**_TRANSPORT)
    sensitive_adapter = ReviewedClinicalTrialsRetrievalAdapter(config, fetch=sensitive_transport)
    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="caller transport failed") as failure:
        sensitive_adapter.execute(sensitive_adapter.prepare(), approve_source_dispatch=True)
    assert "sensitive upstream" not in str(failure.value)
    assert failure.value.__cause__ is None


def test_generic_registration_projects_only_validated_digest_metadata() -> None:
    _, adapter, calls = _adapter([_response([_study()], 1)])
    plan = adapter.prepare()
    registration = create_reviewed_clinical_trials_autonomous_evidence_registration(
        adapter, plan, condition_lane="glioblastoma"
    )
    registry = AutonomousEvidenceAdapterRegistry((registration,))
    metadata = create_reviewed_clinical_trials_execution_metadata(
        plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT
    )
    context = {
        "requirement": {"domain": "biomedical", "label": "trial_registry"},
        "request": {
            "source_id": "clinicaltrials_glioblastoma",
            "source_digest": plan.plan_digest,
            "metadata": metadata,
        },
    }
    value = registry.create_acquirer({"biomedical": registration.adapter_id}).acquire(context)
    observations = registry.create_projector({"biomedical": registration.adapter_id}).project(value, context)
    assert observations == [
        {
            "label": "trial_registry",
            "kind": "provenance",
            "status": "observed",
            "value_digest": value["receipt"]["bundle_digest"],
            "source_digest": value["receipt"]["source_set_digest"],
            "confidence": None,
            "limitations": value["receipt"]["limitations"],
        }
    ]
    assert len(calls) == 1

    forged = value.copy()
    forged["bundle"] = value["bundle"].copy()
    forged["bundle"]["trials"] = [row.copy() for row in value["bundle"]["trials"]]
    forged["bundle"]["trials"][0]["title"] = "tampered"
    with pytest.raises(ReviewedClinicalTrialsRetrievalError):
        registration.project(forged, context)

    with pytest.raises(ReviewedClinicalTrialsRetrievalError, match="single reviewed lane"):
        multi = ReviewedClinicalTrialsRetrievalConfig(
            condition_lanes=("glioblastoma", "glioma"), **_TRANSPORT
        )
        create_reviewed_clinical_trials_autonomous_evidence_registration(
            ReviewedClinicalTrialsRetrievalAdapter(multi, fetch=lambda _url: {}),
            ReviewedClinicalTrialsRetrievalAdapter(multi, fetch=lambda _url: {}).prepare(),
            condition_lane="glioblastoma",
        )


def test_config_and_plan_digests_are_canonical_for_a_shared_fixture() -> None:
    config = ReviewedClinicalTrialsRetrievalConfig(
        condition_lanes=("glioblastoma",),
        page_size=1,
        max_pages=2,
        transport_id="fixture.clinicaltrials",
        transport_version="1",
        transport_config_digest="a" * 64,
    )
    plan = ReviewedClinicalTrialsRetrievalAdapter(config, fetch=lambda _url: {}).prepare()
    assert config.config_digest == content_digest(config.to_dict())
    unsigned = plan.to_dict()
    unsigned.pop("plan_digest")
    assert plan.plan_digest == content_digest(unsigned)
    assert config.config_digest == "459bd696c0262033b456db584620ac2760b3ca3f499e0239d4838dbf32a683d8"
    assert plan.query_set_digest == "6bf9c9f2be2d25041b1aada907c99bab86cef2158524bc8d0ce9d249d085289b"
    assert plan.plan_digest == "27c7b38a6b8ffca018bbd29dd7c6b39ba63e6ebbf158f7df9387808c55561bff"

    result = ReviewedClinicalTrialsRetrievalAdapter(
        config, fetch=lambda _url: _response([_study(title="Reviewed\tregistry\u00a0study")], 1)
    ).execute(
        plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT
    )
    assert result.bundle["trials"][0]["title"] == "Reviewed registry study"
    assert result.bundle["sources"][0]["content_sha256"] == "652627652cb3163c302eb670603cc622b0358d814eb1ad0ca7e019d64b00bc58"
    assert result.bundle["bundle_digest"] == "74d26b1b71989d24e04056ab20fb1a9ba7e9d72c1083dd2d4638ece368016b8e"
    assert result.receipt["receipt_digest"] == "164344a7c80bc3d75c8c0741ba6b5fa61ebf8313622b858860f410ba4b1ece63"
