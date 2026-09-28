from __future__ import annotations

import json

import pytest

import prism_sdk
from prism_sdk.authoring import content_digest
from prism_sdk.autonomous_evidence_adapters import AutonomousEvidenceAdapterRegistry
from prism_sdk.reviewed_open_targets_retrieval import (
    BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST,
    BUILTIN_OPEN_TARGETS_TRANSPORT_ID,
    REVIEWED_OPEN_TARGETS_LANES,
    ReviewedOpenTargetsRetrievalAdapter,
    ReviewedOpenTargetsRetrievalConfig,
    ReviewedOpenTargetsRetrievalError,
    ReviewedOpenTargetsRetrievalPlan,
    create_reviewed_open_targets_autonomous_evidence_registration,
    create_reviewed_open_targets_execution_metadata,
)


_TRANSPORT = {
    "transport_id": "fixture.open-targets",
    "transport_version": "1",
    "transport_config_digest": "a" * 64,
}
_RETRIEVED_AT = "2025-01-02T03:04:05Z"


def _response(lane: str = "gbm", *, total: int = 3, scores: tuple[float, ...] = (0.75, 0.5)) -> dict[str, object]:
    disease_id, disease_name = {
        "gbm": ("MONDO_0018177", "glioblastoma"),
        "lgg": ("MONDO_0021637", "low grade glioma"),
    }[lane]
    targets = [
        {"target": {"id": f"ENSG{index:011d}", "approvedSymbol": symbol}, "score": score}
        for index, (symbol, score) in enumerate(zip(("TP53", "EGFR", "TERT"), scores), start=1)
    ]
    return {"data": {"disease": {"id": disease_id, "name": disease_name, "associatedTargets": {"count": total, "rows": targets}}}}


def _adapter(responses: list[object] | None = None, *, lanes: tuple[str, ...] = ("gbm",), page_size: int = 2):
    values = list(responses or [_response()])
    calls: list[tuple[str, dict[str, object], int]] = []

    def fetch(url: str, body: bytes, timeout_ms: int) -> object:
        calls.append((url, json.loads(body), timeout_ms))
        return values[len(calls) - 1]

    config = ReviewedOpenTargetsRetrievalConfig(lanes=lanes, page_size=page_size, **_TRANSPORT)
    return config, ReviewedOpenTargetsRetrievalAdapter(config, fetch=fetch), calls


def test_config_plan_and_fixed_disease_catalogue_are_digest_bound() -> None:
    assert prism_sdk.ReviewedOpenTargetsRetrievalAdapter is ReviewedOpenTargetsRetrievalAdapter
    assert dict(REVIEWED_OPEN_TARGETS_LANES) == {"gbm": "MONDO_0018177", "lgg": "MONDO_0021637"}
    with pytest.raises(TypeError):
        REVIEWED_OPEN_TARGETS_LANES["other"] = "MONDO_0000000"  # type: ignore[index]
    assert ReviewedOpenTargetsRetrievalConfig().transport_id == BUILTIN_OPEN_TARGETS_TRANSPORT_ID
    assert BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST == ReviewedOpenTargetsRetrievalConfig().transport_config_digest
    assert len(BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST) == 64
    config, adapter, _calls = _adapter()
    plan = adapter.prepare()
    assert ReviewedOpenTargetsRetrievalConfig.from_dict(config.to_dict()) == config
    assert ReviewedOpenTargetsRetrievalPlan.from_dict(plan.to_dict()).to_dict() == plan.to_dict()
    assert plan.to_dict()["request_limit"] == 1
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="unsupported or duplicate"):
        ReviewedOpenTargetsRetrievalConfig(lanes=("other",), **_TRANSPORT)
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="unsupported or duplicate"):
        ReviewedOpenTargetsRetrievalConfig(lanes=("gbm", "gbm"), **_TRANSPORT)
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="invalid shape"):
        ReviewedOpenTargetsRetrievalConfig.from_dict({**config.to_dict(), "query": "arbitrary"})
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="distinct reviewed identity"):
        ReviewedOpenTargetsRetrievalAdapter(ReviewedOpenTargetsRetrievalConfig(), fetch=lambda *_args: {})


def test_approval_pins_graphql_request_and_keeps_truncated_ranking_explicit() -> None:
    response_text = json.dumps(_response(), separators=(",", ":"), ensure_ascii=False)
    _config, adapter, calls = _adapter([response_text])
    plan = adapter.prepare()
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="literal approval"):
        adapter.execute(plan, approve_source_dispatch=1)  # type: ignore[arg-type]
    assert calls == []
    result = adapter.execute(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert _config.config_digest == "7a7f94256ebba8cec8bfce6310f01ddfff0b9d5496b4b33d06f5cb8d377de08a"
    assert plan.plan_digest == "3cba77e6ddc03b360e7c075a55501f9f675371a0236251b9a463205cf0b987ce"
    assert result.receipt["bundle_digest"] == "10a3a1a9f3121644a0b283b834afa11d129e02532fa49fa0f5c4d4abd476499b"
    assert result.receipt["receipt_digest"] == "1472957e7165c725cd59c321a5b176632f4d23fc13fb0b1fd3a483740b9b51d2"
    assert len(calls) == 1
    url, body, timeout_ms = calls[0]
    assert url == "https://api.platform.opentargets.org/api/v4/graphql"
    assert body["query"] == (
        "query ReviewedDiseaseAssociations($efoId: String!, $pageIndex: Int!, $pageSize: Int!) "
        "{ disease(efoId: $efoId) { id name associatedTargets(page: { index: $pageIndex, size: $pageSize }) "
        "{ count rows { target { id approvedSymbol } score } } } }"
    )
    assert body["variables"] == {"efoId": "MONDO_0018177", "pageIndex": 0, "pageSize": 2}
    assert timeout_ms == 30_000
    association = result.bundle["associations"][0]
    assert association["total_associations"] == 3
    assert association["returned_associations"] == 2
    assert association["omitted_associations"] == 1
    assert association["coverage"] == "top_ranked_page_only"
    assert association["targets"][0]["approved_symbol"] == "TP53"
    assert association["targets"][0]["rank"] == 1
    assert association["score_interpretation"] == "ranking_only_not_confidence"
    assert result.receipt["coverage"] == "top_ranked_pages_only"
    assert result.receipt["response_bytes"] == len(response_text.encode("utf-8"))
    assert '"confidence":' not in json.dumps(result.bundle)
    assert result.receipt["request_count"] == 1


def test_two_lane_plan_is_bounded_and_canonical_order_is_stable() -> None:
    _config, adapter, calls = _adapter([_response(), _response("lgg", total=1, scores=(0.25,))], lanes=("lgg", "gbm"))
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    assert [call[1]["variables"]["efoId"] for call in calls] == ["MONDO_0018177", "MONDO_0021637"]
    assert result.receipt["request_count"] == 2
    assert result.bundle["coverage"] == "top_ranked_pages_only"
    assert result.bundle["associations"][1]["coverage"] == "all_source_rows"


def test_duplicate_fields_mismatched_identity_and_invalid_scores_fail_closed() -> None:
    config = ReviewedOpenTargetsRetrievalConfig(page_size=2, **_TRANSPORT)
    duplicate = ReviewedOpenTargetsRetrievalAdapter(config, fetch=lambda *_args: '{"data":{"disease":{"id":"MONDO_0018177","id":"MONDO_0021637"}}}')
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="duplicate JSON fields"):
        duplicate.execute(duplicate.prepare(), approve_source_dispatch=True)
    foreign = ReviewedOpenTargetsRetrievalAdapter(config, fetch=lambda *_args: _response("lgg"))
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="identity differs"):
        foreign.execute(foreign.prepare(), approve_source_dispatch=True)
    invalid = _response()
    invalid["data"]["disease"]["associatedTargets"]["rows"][0]["score"] = 1.5  # type: ignore[index]
    malformed = ReviewedOpenTargetsRetrievalAdapter(config, fetch=lambda *_args: invalid)
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="score is outside"):
        malformed.execute(malformed.prepare(), approve_source_dispatch=True)
    out_of_order = _response(scores=(0.5, 0.75))
    malformed_order = ReviewedOpenTargetsRetrievalAdapter(config, fetch=lambda *_args: out_of_order)
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="not ordered"):
        malformed_order.execute(malformed_order.prepare(), approve_source_dispatch=True)


def test_evidence_registration_validates_review_and_projects_only_digests() -> None:
    _config, adapter, _calls = _adapter()
    plan = adapter.prepare()
    registration = create_reviewed_open_targets_autonomous_evidence_registration(adapter, plan, lane="gbm")
    registry = AutonomousEvidenceAdapterRegistry([registration])
    assert registry.manifests()[0].adapter_id == "reviewed.open_targets.gbm"
    metadata = create_reviewed_open_targets_execution_metadata(plan, approve_source_dispatch=True, retrieved_at=_RETRIEVED_AT)
    request = {"source_id": "open_targets_gbm", "source_digest": plan.plan_digest, "metadata": metadata}
    transient = registration.acquire({"request": request})
    observations = registration.project(transient, {"requirement": {"label": "Open Targets association review"}})
    assert observations[0]["kind"] == "provenance"
    assert observations[0]["value_digest"] == transient["receipt"]["bundle_digest"]
    assert observations[0]["confidence"] is None
    assert "TP53" not in json.dumps(observations)
    assert "association_score" not in json.dumps(observations)
    tampered = json.loads(json.dumps(transient))
    tampered["bundle"]["associations"][0]["targets"][0]["association_score"] = 0.1
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="transient digests are invalid"):
        registration.project(tampered, {"requirement": {"label": "Open Targets association review"}})
    forged = {**metadata, "approve_source_dispatch": False}
    forged["metadata_digest"] = content_digest({key: value for key, value in forged.items() if key != "metadata_digest"})
    with pytest.raises(ReviewedOpenTargetsRetrievalError, match="failed review binding"):
        registration.acquire({"request": {**request, "metadata": forged}})
