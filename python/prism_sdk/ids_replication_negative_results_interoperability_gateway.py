"""Python parity for ``AFA-ids-P15-F22`` replication interoperability."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P15-F22"
CONTRACT_VERSION = "ids-multimodal-multi-study-replication-negative-results-interoperability-gateway/1.0"
INPUT_SCHEMA = "ClaimAndProtocol7@1"
OUTPUT_SCHEMA = "ReplicationRecord9@1"
CONTENT_TYPE = "application/vnd.aurora.replication-record-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ReplicationRecord9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k), str) and v[k].strip() for k in ("request_id", "claim_id", "protocol_id", "semantic_profile")) or int(v.get("checkpoint", 0)) <= 0 or not v.get("observation_order") or not v.get("site_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("replication identity, checkpoint, locality, observations, sites, peers, or effects are incomplete")
        fields = ("observation_order", "qualified_observation_order", "unresolved_observation_order", "blocked_observation_order", "positive_order", "null_order", "negative_order", "inconclusive_order", "site_order", "qualified_site_order", "missing_site_order", "peer_order", "qualified_peer_order", "missing_peer_order", "incomparable_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("replication ordering is not canonical")
        observations = set(v["observation_order"]); states = [*v["qualified_observation_order"], *v["unresolved_observation_order"], *v["blocked_observation_order"]]; outcomes = [*v["positive_order"], *v["null_order"], *v["negative_order"], *v["inconclusive_order"]]; sites = set(v["site_order"]); site_states = [*v["qualified_site_order"], *v["missing_site_order"]]; peers = set(v["peer_order"]); peer_states = [*v["qualified_peer_order"], *v["missing_peer_order"]]
        if len(observations) != len(v["observation_order"]) or set(states) != observations or len(set(states)) != len(states) or set(outcomes) != set(v["qualified_observation_order"]) or len(outcomes) != len(v["qualified_observation_order"]) or set(site_states) != sites or len(set(site_states)) != len(site_states) or set(peer_states) != peers or len(set(peer_states)) != len(peer_states) or v.get("positive_count") != len(v["positive_order"]) or v.get("null_count") != len(v["null_order"]) or v.get("negative_count") != len(v["negative_order"]) or v.get("inconclusive_count") != len(v["inconclusive_order"]):
            raise ResearchContractError("replication observation, outcome, site, peer, or count states do not partition")
        artifact = v.get("artifact", {}); digests = [v.get("replay_identity"), v.get("record_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", [])]
        if not all(_digest(x) for x in digests) or artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_hash") != v.get("record_digest"):
            raise ResearchContractError("replication artifact metadata or digest is inconsistent")
        if any(not e.startswith(("exchange:permitted-summaries:", "manage:local-capability:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("effect is outside the replication interoperability gate")


def replication_interoperability_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["computational biologist", "replication coordinator", "multimodal integration gateway", "federation steward"], "behavior": "validates multimodal multi-study replication attestations and emits an interoperable negative-results record", "value": "makes positive, null, negative, inconclusive, incomparable, and contradictory replication outcomes exchangeable without erasing uncertainty", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:permitted-summaries", "manage:local-capability"], "permissions": ["read:local-replication-manifests", "exchange:aggregate-results"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    claim = request.get("claim", {})
    if not isinstance(claim, Mapping) or not isinstance(request.get("request_id"), str) or not request["request_id"].strip() or not request.get("observations") or len(request["observations"]) > 8192 or not request.get("peers") or len(request["peers"]) > 1024 or int(request.get("checkpoint", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("max_budget_units", 0)) <= 0 or not all(isinstance(claim.get(k), str) and claim[k].strip() for k in ("claim_id", "protocol_id", "semantic_profile")) or not claim.get("study_ids") or claim["study_ids"] != sorted(set(claim["study_ids"])) or not claim.get("modality_ids") or claim["modality_ids"] != sorted(set(claim["modality_ids"])) or int(claim.get("minimum_replicates", 0)) <= 0 or not 0 <= int(claim.get("effect_threshold_milli", -1)) <= 1000 or not all(_digest(claim.get(k)) for k in ("claim_digest", "provenance_digest", "replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("request, claim, study/modality closure, bounds, replay, locality, or boundary is invalid")
    ids: set[str] = set()
    for o in request["observations"]:
        if not all(isinstance(o.get(k), str) and o[k].strip() for k in ("observation_id", "site_id", "study_id", "outcome")) or o["observation_id"] in ids or not o.get("modality_ids") or o["modality_ids"] != sorted(set(o["modality_ids"])) or o["outcome"] not in {"positive", "null", "negative", "inconclusive"} or not 0 <= int(o.get("uncertainty_milli", -1)) <= 1000 or int(o.get("estimated_units", 0)) <= 0 or not all(_digest(o.get(k)) for k in ("artifact_digest", "provenance_digest", "replay_identity")) or o.get("omission_reasons", []) != sorted(set(o.get("omission_reasons", []))):
            raise ResearchContractError("observation identity, modalities, outcome, bounds, omissions, or digests are invalid")
        ids.add(o["observation_id"])
    peer_ids: set[str] = set()
    for p in request["peers"]:
        if not all(isinstance(p.get(k), str) and p[k].strip() for k in ("peer_id", "origin", "claim_id", "semantic_profile")) or p["peer_id"] in peer_ids or int(p.get("checkpoint", 0)) <= 0 or int(p.get("observation_count", 0)) <= 0 or not _digest(p.get("replication_digest")):
            raise ResearchContractError("peer identity, claim, checkpoint, observation count, or digest is invalid")
        peer_ids.add(p["peer_id"])


def interoperate_replication(request: Mapping[str, Any]) -> ReplicationRecord9:
    _validate_request(request)
    claim = dict(request["claim"]); observations = sorted((dict(x) for x in request["observations"]), key=lambda x: x["observation_id"]); peers = sorted((dict(x) for x in request["peers"]), key=lambda x: x["peer_id"]); ids = [x["observation_id"] for x in observations]; pids = [x["peer_id"] for x in peers]
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); positive: set[str] = set(); null_results: set[str] = set(); negative: set[str] = set(); inconclusive: set[str] = set(); incomparable: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative_evidence: set[str] = set(); total = 0
    effects: list[int] = []
    for o in observations:
        oid = o["observation_id"]; total += int(o["estimated_units"])
        if o.get("negative_result"): negative_evidence.add(f"{oid}:negative-result")
        if o.get("evidence_state") == "contradicted": blocked.add(oid); negative_evidence.add(f"{oid}:contradicted"); continue
        study_ok = o["study_id"] in claim["study_ids"]; modality_ok = all(m in o["modality_ids"] for m in claim["modality_ids"])
        if not study_ok or not modality_ok or o.get("raw_data_local") is not True or o.get("aggregate_only") is not True: blocked.add(oid); omissions.add(f"{oid}:study-modality-or-locality-closure"); continue
        if o.get("replay_identity") != claim["replay_identity"] or o.get("signed") is not True or o.get("permitted") is not True: unresolved.add(oid); omissions.add(f"{oid}:replay-or-authorization"); continue
        if o.get("evidence_state") not in {"proven", "supported"}: unresolved.add(oid); uncertainty.add(f"{oid}:evidence-state"); continue
        if o.get("comparable") is not True: unresolved.add(oid); incomparable.add(oid); uncertainty.add(f"{oid}:cross-study-comparability"); continue
        if o["outcome"] == "positive" and abs(int(o["effect_milli"])) < int(claim["effect_threshold_milli"]): unresolved.add(oid); uncertainty.add(f"{oid}:effect-threshold"); omissions.add(f"{oid}:positive-effect-below-registered-threshold"); continue
        qualified.add(oid); effects.append(int(o["effect_milli"]))
        {"positive": positive, "null": null_results, "negative": negative, "inconclusive": inconclusive}[o["outcome"]].add(oid)
        if o["outcome"] in {"null", "negative"}: negative_evidence.add(f"{oid}:{'null-result' if o['outcome']=='null' else 'negative-outcome'}")
        if o["outcome"] == "inconclusive": uncertainty.add(f"{oid}:inconclusive")
        omissions.update(f"{oid}:{r}" for r in o.get("omission_reasons", []))
    sites = sorted({o["site_id"] for o in observations}); qualified_sites = sorted({o["site_id"] for o in observations if o["observation_id"] in qualified}); missing_sites = sorted(set(sites) - set(qualified_sites)); omissions.add("site:qualified-closure-incomplete") if missing_sites else None
    qualified_peers = {p["peer_id"] for p in peers if p["claim_id"] == claim["claim_id"] and p["semantic_profile"] == claim["semantic_profile"] and int(p["checkpoint"]) == int(request["checkpoint"]) and p.get("observation_count", 0) > 0 and p.get("signed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and p.get("evidence_state") in {"proven", "supported"}}; missing_peers = set(pids) - qualified_peers; uncertainty.update(f"peer:{p}:not-qualified" for p in missing_peers)
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    if len(qualified) < int(claim["minimum_replicates"]): uncertainty.add("replication:minimum-replicates-unmet")
    if total > int(request["max_budget_units"]): omissions.add(f"request:budget-exceeded:{total}")
    if request.get("policy_allow") is not True: negative_evidence.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block: blocked.update(ids); qualified.clear(); unresolved.clear(); positive.clear(); null_results.clear(); negative.clear(); inconclusive.clear(); omissions.add("request:replication-interoperability-not-authorized")
    disposition = "blocked" if global_block or (not qualified and blocked) else "unresolved" if len(qualified) < int(claim["minimum_replicates"]) or len(qualified_peers) < int(request["minimum_peer_quorum"]) or total > int(request["max_budget_units"]) else "qualified"
    if disposition != "qualified": omissions.add("request:replication-record-not-release-ready")
    qorder = sorted(qualified); uorder = sorted(unresolved); border = sorted(blocked); median_available = bool(effects); med = sorted(effects)[(len(effects) - 1) // 2] if effects else 0
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "claim_id": claim["claim_id"], "protocol_id": claim["protocol_id"], "semantic_profile": claim["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "observation_order": ids, "qualified_observation_order": qorder, "unresolved_observation_order": uorder, "blocked_observation_order": border, "positive_order": sorted(positive), "null_order": sorted(null_results), "negative_order": sorted(negative), "inconclusive_order": sorted(inconclusive), "site_order": sites, "qualified_site_order": qualified_sites, "missing_site_order": missing_sites, "peer_order": pids, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "incomparable_order": sorted(incomparable), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative_evidence), "effect_median_milli": med, "effect_median_available": median_available, "positive_count": len(positive), "null_count": len(null_results), "negative_count": len(negative), "inconclusive_count": len(inconclusive), "total_units": total, "replay_identity": claim["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); result = {**payload, "record_digest": digest, "artifact": {"artifact_id": f"replication-record-9:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": sorted(omissions), "provenance_digests": sorted({claim["provenance_digest"], *(o["provenance_digest"] for o in observations)}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": sorted([f"exchange:permitted-summaries:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]), "raw_data_local": True, "aggregate_only": True}; receipt = ReplicationRecord9(result); receipt.validate(); return receipt


def idsReplicationInteroperabilityDigest(receipt: ReplicationRecord9) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "ReplicationRecord9", "replication_interoperability_manifest", "interoperate_replication", "idsReplicationInteroperabilityDigest"]
