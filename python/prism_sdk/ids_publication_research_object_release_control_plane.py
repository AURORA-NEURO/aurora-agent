"""Python parity for ``AFA-ids-P16-F31`` publication/release control."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P16-F31"
CONTRACT_VERSION = "ids-prospective-high-throughput-publication-research-object-release-federated-control-plane/1.0"
INPUT_SCHEMA = "ValidatedResearchRun7@1"
OUTPUT_SCHEMA = "SignedResearchObject11@1"
CONTENT_TYPE = "application/vnd.aurora.signed-research-object-11+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class SignedResearchObject11:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k), str) and v[k].strip() for k in ("request_id", "run_id", "requester", "purpose", "semantic_profile", "engine_version")) or int(v.get("checkpoint", 0)) <= 0 or not v.get("artifact_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("release identity, checkpoint, locality, artifacts, peers, or effects are incomplete")
        fields = ("artifact_order", "selected_artifact_order", "unresolved_artifact_order", "blocked_artifact_order", "missing_provenance_order", "missing_evidence_order", "omitted_field_order", "negative_result_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("release ordering is not canonical")
        artifacts = set(v["artifact_order"]); states = [*v["selected_artifact_order"], *v["unresolved_artifact_order"], *v["blocked_artifact_order"]]; peers = set(v["peer_order"]); peer_states = [*v["qualified_peer_order"], *v["missing_peer_order"]]
        if len(artifacts) != len(v["artifact_order"]) or set(states) != artifacts or len(set(states)) != len(states) or set(peer_states) != peers or len(set(peer_states)) != len(peer_states):
            raise ResearchContractError("artifact or peer states do not partition")
        a = v.get("artifact", {}); digests = [v.get("replay_identity"), v.get("release_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]
        if not all(_digest(x) for x in digests) or a.get("content_type") != CONTENT_TYPE or a.get("boundary") != PRECLINICAL_BOUNDARY or a.get("content_hash") != v.get("release_digest"):
            raise ResearchContractError("release artifact metadata or digest is inconsistent")
        if any(not e.startswith(("exchange:permitted-summaries:", "manage:local-capability:", "publish:signed-research-object:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("effect is outside the governed release gate")


def publication_release_control_plane_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["computational biologist", "research-object steward", "publication gateway operator", "federation steward"], "behavior": "compiles high-throughput validated research-run manifests into a deterministic signed release intent", "value": "prevents missing provenance, evidence, replay, policy, approval, peer, budget, or locality closure from becoming an apparently publishable research object", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:permitted-summaries", "manage:local-capability", "publish:signed-research-object"], "permissions": ["read:local-research-object-manifests", "request:governed-release"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "run_id", "requester", "purpose", "semantic_profile", "engine_version")) or not request.get("artifacts") or len(request["artifacts"]) > 8192 or not request.get("peers") or len(request["peers"]) > 1024 or int(request.get("checkpoint", 0)) <= 0 or int(request.get("minimum_artifact_count", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("max_budget_units", 0)) <= 0 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("request identity, artifacts, peers, bounds, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for a in request["artifacts"]:
        if not all(isinstance(a.get(k), str) and a[k].strip() for k in ("artifact_id", "run_id", "study_id", "semantic_profile", "media_type")) or a["artifact_id"] in ids or int(a.get("estimated_units", 0)) <= 0 or not all(_digest(a.get(k)) for k in ("content_digest", "provenance_digest", "evidence_digest", "replay_identity")) or a.get("omitted_fields", []) != sorted(set(a.get("omitted_fields", []))):
            raise ResearchContractError("research artifact identity, digest, bounds, or omission ordering is invalid")
        ids.add(a["artifact_id"])
    peer_ids: set[str] = set()
    for p in request["peers"]:
        if not all(isinstance(p.get(k), str) and p[k].strip() for k in ("peer_id", "origin", "run_id", "semantic_profile")) or p["peer_id"] in peer_ids or int(p.get("checkpoint", 0)) <= 0 or int(p.get("artifact_count", 0)) <= 0 or not _digest(p.get("object_digest")):
            raise ResearchContractError("release peer identity, checkpoint, count, or digest is invalid")
        peer_ids.add(p["peer_id"])


def compile_publication_release(request: Mapping[str, Any]) -> SignedResearchObject11:
    _validate_request(request)
    artifacts = sorted((dict(x) for x in request["artifacts"]), key=lambda x: x["artifact_id"]); peers = sorted((dict(x) for x in request["peers"]), key=lambda x: x["peer_id"]); ids = [x["artifact_id"] for x in artifacts]; pids = [x["peer_id"] for x in peers]
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); missing_provenance: set[str] = set(); missing_evidence: set[str] = set(); omitted_fields: set[str] = set(); negative_result: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative_evidence: set[str] = set(); total = 0
    for a in artifacts:
        aid = a["artifact_id"]; total += int(a["estimated_units"])
        if a.get("negative_result"): negative_result.add(aid); negative_evidence.add(f"{aid}:negative-result")
        if a["run_id"] != request["run_id"] or a["semantic_profile"] != request["semantic_profile"]: blocked.add(aid); omissions.add(f"{aid}:run-or-semantic-profile-mismatch"); continue
        if a.get("raw_data_local") is not True or a.get("aggregate_only") is not True: blocked.add(aid); omissions.add(f"{aid}:raw-data-not-local-or-aggregate"); continue
        if a.get("protected_closure") is not True: blocked.add(aid); uncertainty.add(f"{aid}:protected-closure-incomplete"); continue
        if a.get("evidence_state") == "contradicted": blocked.add(aid); negative_evidence.add(f"{aid}:contradicted"); continue
        if not _digest(a.get("provenance_digest")): missing_provenance.add(aid)
        if not _digest(a.get("evidence_digest")): missing_evidence.add(aid)
        omitted_fields.update(f"{aid}:{x}" for x in a.get("omitted_fields", []))
        if a.get("replay_identity") != request["replay_identity"] or a.get("permitted") is not True or a.get("signed") is not True: unresolved.add(aid); omissions.add(f"{aid}:replay-or-authorization"); continue
        if a.get("evidence_state") not in {"proven", "supported"}: unresolved.add(aid); uncertainty.add(f"{aid}:evidence-state"); continue
        if aid not in missing_provenance and aid not in missing_evidence and not a.get("omitted_fields"):
            selected.add(aid)
        else:
            unresolved.add(aid); omissions.add(f"{aid}:closure-or-omission")
    if missing_provenance: omissions.add("request:provenance-closure-incomplete")
    if missing_evidence: omissions.add("request:evidence-closure-incomplete")
    if omitted_fields: omissions.add("request:explicit-semantic-loss-present")
    qualified_peers = {p["peer_id"] for p in peers if p["run_id"] == request["run_id"] and p["semantic_profile"] == request["semantic_profile"] and int(p["checkpoint"]) == int(request["checkpoint"]) and int(p["artifact_count"]) > 0 and p.get("signed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and p.get("evidence_state") in {"proven", "supported"}}; missing_peers = set(pids) - qualified_peers; uncertainty.update(f"peer:{p}:not-qualified" for p in missing_peers)
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    if len(selected) < int(request["minimum_artifact_count"]): uncertainty.add("release:minimum-artifact-count-unmet")
    if total > int(request["max_budget_units"]): omissions.add(f"request:budget-exceeded:{total}")
    if request.get("policy_allow") is not True: negative_evidence.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(ids); selected.clear(); unresolved.clear(); omissions.add("request:publication-release-not-authorized")
    disposition = "blocked" if global_block or (not selected and blocked) else "unresolved" if len(selected) < int(request["minimum_artifact_count"]) or len(qualified_peers) < int(request["minimum_peer_quorum"]) or total > int(request["max_budget_units"]) else "qualified"
    if disposition != "qualified": omissions.add("request:release-intent-not-release-ready")
    sorder = sorted(selected); uorder = sorted(unresolved); border = sorted(blocked)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "run_id": request["run_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "engine_version": request["engine_version"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "artifact_order": ids, "selected_artifact_order": sorder, "unresolved_artifact_order": uorder, "blocked_artifact_order": border, "missing_provenance_order": sorted(missing_provenance), "missing_evidence_order": sorted(missing_evidence), "omitted_field_order": sorted(omitted_fields), "negative_result_order": sorted(negative_result), "peer_order": pids, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative_evidence), "total_units": total, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); result = {**payload, "release_digest": digest, "artifact": {"artifact_id": f"signed-research-object-11:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(omissions), "provenance_digests": sorted({a["provenance_digest"] for a in artifacts}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": sorted([f"exchange:permitted-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}", f"publish:signed-research-object:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]), "raw_data_local": True, "aggregate_only": True}; receipt = SignedResearchObject11(result); receipt.validate(); return receipt


def idsPublicationReleaseDigest(receipt: SignedResearchObject11) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "SignedResearchObject11", "publication_release_control_plane_manifest", "compile_publication_release", "idsPublicationReleaseDigest"]
