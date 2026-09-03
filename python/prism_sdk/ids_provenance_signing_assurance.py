"""Python parity for ``AFA-ids-P18-F26`` provenance/signing assurance."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P18-F26"
CONTRACT_VERSION = "ids-multimodal-provenance-signing-assurance/1.0"
INPUT_SCHEMA = "ProvenanceBundleRequest7@1"
OUTPUT_SCHEMA = "SignedProvenanceReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.signed-provenance-receipt-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class SignedProvenanceReceipt9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("artifact", {}).get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("signature_mode") != "detached-digest-attestation" or not v.get("node_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("provenance identity, locality, signature mode, nodes, disposition, or effects are incomplete")
        fields = ("node_order", "verified_order", "unresolved_order", "blocked_order", "missing_parent_order", "cycle_order", "invalid_signature_order", "root_mismatch_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("provenance receipt ordering is not canonical")
        ids = set(v["node_order"]); states = v["verified_order"] + v["unresolved_order"] + v["blocked_order"]
        if len(ids) != len(v["node_order"]) or len(states) != len(ids) or set(states) != ids:
            raise ResearchContractError("provenance node states do not partition")
        a = v.get("artifact", {}); digests = [v.get("root_digest"), v.get("replay_identity"), v.get("receipt_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]
        if not all(_digest(d) for d in digests) or a.get("content_type") != CONTENT_TYPE or a.get("content_hash") != v.get("receipt_digest"):
            raise ResearchContractError("provenance digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:provenance-digests:", "manage:local-capability:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("provenance effect is outside governed gate")


def provenance_signing_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["research-object steward", "provenance auditor", "federation operator", "replay auditor"], "behavior": "verifies local multimodal provenance DAGs, signatures, roots, and semantic closure", "value": "prevents missing lineage, cycles, invalid signatures, or root drift from becoming reproducible research-object claims", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:provenance-digests", "manage:local-capability"], "permissions": ["read:local-provenance", "request:provenance-assurance"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "artifact_id", "semantic_profile")) or not request.get("nodes") or len(request["nodes"]) > 16384 or not _digest(request.get("expected_root")) or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("provenance request identity, nodes, bounds, roots, replay, or locality is invalid")
    ids: set[str] = set()
    for node in request["nodes"]:
        if not all(isinstance(node.get(k), str) and node[k].strip() for k in ("node_id", "kind", "actor")) or not _digest(node.get("content_digest")) or node["node_id"] in ids:
            raise ResearchContractError("provenance node identity, actor, digest, or uniqueness is invalid")
        ids.add(node["node_id"])


def assure_provenance_signing(request: Mapping[str, Any]) -> SignedProvenanceReceipt9:
    _validate_request(request)
    nodes = sorted((dict(n) for n in request["nodes"]), key=lambda n: n["node_id"]); ids = [n["node_id"] for n in nodes]; by_id = {n["node_id"]: n for n in nodes}
    missing_parent: set[str] = set(); invalid_signature: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); cycle: set[str] = set(); root_mismatch: set[str] = set(); omissions: set[str] = set()
    for n in nodes:
        missing_parent.update(f"{n['node_id']}:{p}" for p in n.get("parent_ids", []) if p not in by_id)
        if n.get("signature_valid") is not True: invalid_signature.add(n["node_id"])
        if n.get("evidence_state") == "contradicted": negative.add(f"{n['node_id']}:contradicted")
        elif n.get("evidence_state") not in {"proven", "supported"}: uncertainty.add(f"{n['node_id']}:evidence-state")
    indegree = {n["node_id"]: sum(p in by_id for p in n.get("parent_ids", [])) for n in nodes}; children: dict[str, list[str]] = {i: [] for i in ids}
    for n in nodes:
        for p in n.get("parent_ids", []):
            if p in by_id: children[p].append(n["node_id"])
    queue = sorted(i for i, degree in indegree.items() if degree == 0); topo: list[str] = []
    while queue:
        i = queue.pop(0); topo.append(i)
        for child in sorted(children[i]):
            indegree[child] -= 1
            if indegree[child] == 0: queue.append(child); queue.sort()
    cycle = set(ids) - set(topo)
    verified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set()
    for n in nodes:
        i = n["node_id"]
        if i in cycle: blocked.add(i); omissions.add(f"{i}:provenance-cycle")
        elif any(p not in by_id for p in n.get("parent_ids", [])): unresolved.add(i)
        elif not n.get("local") or not n.get("protected") or not n.get("signature_valid"): blocked.add(i)
        elif n.get("evidence_state") not in {"proven", "supported"}: unresolved.add(i)
        else: verified.add(i)
        if n.get("content_digest") == request["expected_root"] and n.get("parent_ids"): root_mismatch.add(i)
    if not any(n["content_digest"] == request["expected_root"] and not n.get("parent_ids") for n in nodes):
        root_mismatch.add(request["artifact_id"]); omissions.add("request:expected-root-not-found")
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only")) or bool(root_mismatch)
    if global_block: blocked.update(ids); verified.clear(); unresolved.clear(); omissions.add("request:provenance-governance-or-root-denied")
    vo, uo, bo = sorted(verified), sorted(unresolved), sorted(blocked); disposition = "blocked" if global_block or (not vo and not uo) else "unresolved" if uo or bo else "qualified"
    if disposition != "qualified": omissions.add("request:provenance-closure-incomplete")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "artifact_id": request["artifact_id"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "node_order": ids, "verified_order": vo, "unresolved_order": uo, "blocked_order": bo, "missing_parent_order": sorted(missing_parent), "cycle_order": sorted(cycle), "invalid_signature_order": sorted(invalid_signature), "root_mismatch_order": sorted(root_mismatch), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "root_digest": request["expected_root"], "replay_identity": request["replay_identity"], "signature_mode": "detached-digest-attestation", "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    rd = _hash(payload); payload["receipt_digest"] = rd; payload["artifact"] = {"artifact_id": f"signed-provenance-receipt-9:{request['artifact_id']}", "content_type": CONTENT_TYPE, "content_hash": rd, "semantic_loss": sorted(omissions), "provenance_digests": sorted({n["content_digest"] for n in nodes}), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = sorted([f"exchange:provenance-digests:{request['request_id']}", f"manage:local-capability:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"])
    receipt = SignedProvenanceReceipt9(payload); receipt.validate(); return receipt


def idsProvenanceSigningDigest(receipt: SignedProvenanceReceipt9) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "SignedProvenanceReceipt9", "provenance_signing_assurance_manifest", "assure_provenance_signing", "idsProvenanceSigningDigest"]
