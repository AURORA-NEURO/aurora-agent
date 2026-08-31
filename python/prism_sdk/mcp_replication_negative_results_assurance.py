"""Python parity surface for ``AFA-mcp-P15-F27``.

The implementation is an evidence/release gate over aggregate replication summaries. It never
reruns protocols, reads raw measurements, exports protected data, or makes clinical decisions.
"""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-mcp-P15-F27"
CONTRACT_VERSION = "mcp-prospective-high-throughput-replication-negative-results-assurance/1.0"
INPUT_SCHEMA = "ClaimAndProtocol3@1"
OUTPUT_SCHEMA = "ReplicationRecord7@1"
CONTENT_TYPE = "application/vnd.aurora.mcp-replication-record-7+json"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: Sequence[str]) -> bool:
    return list(values) == sorted(set(values))

@dataclass(frozen=True)
class ReplicationRecord7:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not str(v.get("request_id", "")).strip() or not str(v.get("claim_id", "")).strip() or not v.get("observation_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("replication identity, locality, observations, or effects are incomplete")
        fields = ("observation_order", "qualified_order", "unresolved_order", "blocked_order", "positive_order", "null_order", "negative_order", "inconclusive_order", "site_order", "missing_site_order", "omission_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields): raise ResearchContractError("replication ordering is not canonical")
        if set(v["observation_order"]) != set(v["qualified_order"]) | set(v["unresolved_order"]) | set(v["blocked_order"]): raise ResearchContractError("replication observations do not partition")
        a = v.get("artifact", {})
        if not all(_digest(x) for x in (v.get("replay_identity"), v.get("record_digest"), a.get("content_hash"))) or a.get("content_type") != CONTENT_TYPE or a.get("boundary") != PRECLINICAL_BOUNDARY or a.get("content_hash") != v.get("record_digest"): raise ResearchContractError("replication artifact or digest is invalid")
        if any(e != "block:unsafe-release" and not e.startswith("release:replication:") for e in v["effect_receipts"]): raise ResearchContractError("replication effect is outside governed gate")

def replication_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "mcp", "consumers": ["AURORA extension developer", "replication scientist", "release governance board"], "behavior": "audits high-throughput replication and negative-result summaries without executing protocols or exporting raw data", "value": "keeps null, negative, contradictory, and incomplete replication evidence visible before release", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["block:unsafe-release", "release:replication:qualified"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

def assure_replication(request: Mapping[str, Any]) -> ReplicationRecord7:
    required = ("request_id", "claim_id", "claim_text", "protocol_id", "semantic_profile", "expected_direction")
    if not all(str(request.get(k, "")).strip() for k in required) or int(request.get("minimum_replicates", 0)) <= 0 or int(request.get("batch_limit", 0)) <= 0 or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not all(_digest(request.get(k)) for k in ("protocol_digest", "baseline_digest", "replay_identity")): raise ResearchContractError("claim, batch, digest, locality, or boundary constraints are invalid")
    rows = sorted((dict(x) for x in request.get("observations", [])), key=lambda x: (str(x.get("study_id", "")), str(x.get("site_id", "")), str(x.get("observation_id", ""))))
    if not rows or len(rows) > int(request["batch_limit"]) or len(rows) > 16384: raise ResearchContractError("observation batch is empty or exceeds its declared bound")
    ids = [str(x.get("observation_id", "")) for x in rows]
    if len(set(ids)) != len(ids) or any(not x.get("observation_id") or not x.get("study_id") or not x.get("site_id") or not all(_digest(x.get(k)) for k in ("artifact_digest", "provenance_digest", "replay_identity")) for x in rows): raise ResearchContractError("observation identity or digest is invalid")
    q: set[str] = set(); u: set[str] = set(); b: set[str] = set(); pos: set[str] = set(); nul: set[str] = set(); neg: set[str] = set(); inc: set[str] = set(); om: set[str] = set(); ne: set[str] = set(); values: list[int] = []
    for x in rows:
        oid, outcome = x["observation_id"], x.get("outcome")
        om.update(f"{oid}:{r}" for r in x.get("omission_reasons", []))
        if outcome == "positive": pos.add(oid); values.append(int(x.get("effect_milli", 0)))
        elif outcome == "null": nul.add(oid); ne.add(f"{oid}:null")
        elif outcome == "negative": neg.add(oid); ne.add(f"{oid}:negative")
        elif outcome == "inconclusive": inc.add(oid)
        compatible = x.get("protocol_id") == request["protocol_id"] and x.get("semantic_profile") == request["semantic_profile"] and x.get("replay_identity") == request["replay_identity"] and x.get("signed") is True and x.get("comparable") is True and x.get("raw_data_local") is True and x.get("aggregate_only") is True
        if x.get("evidence_state") == "contradicted": b.add(oid); ne.add(f"{oid}:contradicted")
        elif not compatible or x.get("evidence_state") not in {"proven", "supported"}: u.add(oid)
        else: q.add(oid)
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only"))
    if request.get("policy_allow") is not True: om.add("request:policy-denied")
    if request.get("protected_closure") is not True: om.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: om.add("request:signed-approval-missing")
    disposition = "blocked" if global_block or b else "unresolved" if len(q) < int(request["minimum_replicates"]) or neg or nul or inc else "qualified"
    if disposition != "qualified": om.add("request:replication-gates-incomplete")
    if global_block: b.update(ids); q.clear(); u.clear()
    qo, uo, bo = sorted(q), sorted(u), sorted(b); sites = sorted({x["site_id"] for x in rows}); qsites = sorted({x["site_id"] for x in rows if x["observation_id"] in q}); values.sort(); median = values[len(values) // 2] if values else 0
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "claim_id": request["claim_id"], "disposition": disposition, "observation_order": ids, "qualified_order": qo, "unresolved_order": uo, "blocked_order": bo, "positive_order": sorted(pos), "null_order": sorted(nul), "negative_order": sorted(neg), "inconclusive_order": sorted(inc), "site_order": sites, "missing_site_order": sorted(set(sites) - set(qsites)), "omission_order": sorted(om), "negative_evidence_order": sorted(ne), "effect_median_milli": median, "positive_count": len(pos), "null_count": len(nul), "negative_count": len(neg), "batch_limit": int(request["batch_limit"]), "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    record_digest = _hash(payload); effects = [f"release:replication:qualified:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    result = {**payload, "record_digest": record_digest, "artifact": {"artifact_id": f"replication-record-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": record_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True}
    receipt = ReplicationRecord7(result); receipt.validate(); return receipt

def mcpReplicationAssuranceDigest(receipt: ReplicationRecord7) -> str:
    receipt.validate(); return _hash(receipt.to_dict())

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "ReplicationRecord7", "replication_assurance_manifest", "assure_replication", "mcpReplicationAssuranceDigest"]
