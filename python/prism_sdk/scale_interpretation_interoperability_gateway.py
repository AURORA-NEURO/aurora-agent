"""Parity implementation for ``AFA-scale-P14-F22``."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-scale-P14-F22"
CONTRACT_VERSION = "scale-multimodal-interpretation-visualization-interoperability-gateway/1.0"
INPUT_SCHEMA = "EvidenceBackedResult2@1"
OUTPUT_SCHEMA = "InteractiveInterpretation6@1"
CONTENT_TYPE = "application/vnd.aurora.scale-interactive-interpretation-6+json"
def _hash(v: Any) -> str: return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v: Any) -> bool: return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v: list[str]) -> bool: return v == sorted(set(v))
def interpretation_interoperability_gateway_manifest() -> dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"scale","consumers":["research workflow operator","federation steward","visualization client"],"behavior":"negotiate and validate versioned multimodal interpretation exchange with semantic-loss and policy witnesses","value":"lets independent research sites exchange compatible interpretation artifacts without exporting raw data or hiding semantic loss","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["federation-export","write:local-artifact"],"permissions":["connect:approved-endpoints"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r: Mapping[str,Any]) -> None:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","required_protocol","required_schema_version","semantic_profile")) or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or not r.get("endpoints"): raise ResearchContractError("identity, policy, locality, replay, or boundary is invalid")
    ids:set[str]=set()
    for e in r["endpoints"]:
        if not isinstance(e.get("endpoint_id"),str) or not e["endpoint_id"].strip() or e["endpoint_id"] in ids or not _digest(e.get("artifact_digest")) or not _digest(e.get("provenance_digest")) or e.get("replay_identity")!=r["replay_identity"] or e.get("local_only") is not True or e.get("aggregate_only") is not True or not _ordered(e.get("semantic_loss_order",[])): raise ResearchContractError("endpoint identity, digest, replay, locality, or ordering is invalid")
        ids.add(e["endpoint_id"])
def validate_interpretation_interoperability(output: Mapping[str,Any]) -> None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("endpoint_order") or not output.get("effect_receipts"): raise ResearchContractError("identity, locality, endpoints, disposition, or effects are incomplete")
    fields=("endpoint_order","compatible_order","unresolved_order","blocked_order","migration_order","semantic_loss_order","negative_evidence_order","effect_receipts")
    if any(not _ordered(output.get(k,[])) for k in fields): raise ResearchContractError("interpretation interoperability ordering is not canonical")
    ids=set(output["endpoint_order"]); parts=sum((output.get(k,[]) for k in ("compatible_order","unresolved_order","blocked_order")),[])
    if len(ids)!=len(output["endpoint_order"]) or len(parts)!=len(ids) or set(parts)!=ids: raise ResearchContractError("endpoint states do not partition")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("interpretation_digest")) or a.get("content_hash")!=output.get("interpretation_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])): raise ResearchContractError("interpretation digest is inconsistent")
    if any(v!="block:unsafe-release" and not v.startswith("exchange:permitted-artifacts:") for v in output["effect_receipts"]): raise ResearchContractError("exchange effect is outside gateway bounds")
    if output["disposition"]=="qualified" and output["effect_receipts"]!=[f"exchange:permitted-artifacts:{output['request_id']}"]: raise ResearchContractError("qualified exchange effect is invalid")
    if output["disposition"]!="qualified" and output["effect_receipts"]!=["block:unsafe-release"]: raise ResearchContractError("non-qualified exchange must block")
def interoperate_interpretations(r: Mapping[str,Any]) -> dict[str,Any]:
    _validate_request(r); rows=sorted((dict(e) for e in r["endpoints"]),key=lambda e:e["endpoint_id"]); endpoint_order=[e["endpoint_id"] for e in rows]; compatible:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); migration:set[str]=set(); semantic_loss:set[str]=set(); negative:set[str]=set(); provenance:set[str]=set()
    for e in rows:
        i=e["endpoint_id"]; provenance.add(e["provenance_digest"]); semantic_loss.update(f"{i}:{x}" for x in e.get("semantic_loss_order",[]));
        if e.get("negative_result"): negative.add(i)
        exact=e.get("protocol")==r["required_protocol"] and e.get("schema_version")==r["required_schema_version"] and e.get("semantic_profile")==r["semantic_profile"] and e.get("comparable") and e.get("policy_allowed") and e.get("authorized"); additive=e.get("protocol")==r["required_protocol"] and e.get("semantic_profile")==r["semantic_profile"] and e.get("comparable") and e.get("policy_allowed") and e.get("authorized")
        if exact and e.get("evidence_state") in {"proven","supported"}: compatible.add(i)
        elif additive and e.get("evidence_state") in {"proven","supported"}: compatible.add(i); migration.add(f"{i}:schema-migration")
        elif not e.get("policy_allowed") or not e.get("authorized") or not e.get("local_only") or not e.get("aggregate_only") or e.get("replay_identity")!=r["replay_identity"]: blocked.add(i); semantic_loss.add(f"{i}:policy-locality-or-replay")
        else: unresolved.add(i); semantic_loss.add(f"{i}:incompatible-or-uncertain")
    global_block=not all(r.get(k) is True for k in ("policy_allowed","protected_closure","raw_data_local","aggregate_only")); disposition="blocked" if global_block or blocked else "partial" if unresolved or not compatible else "qualified"
    if global_block: blocked.update(endpoint_order); compatible.clear(); unresolved.clear(); semantic_loss.add("request:global-gate-blocked")
    if disposition!="qualified": semantic_loss.add("request:exchange-closure-not-ready")
    payload={"endpoint_order":endpoint_order,"compatible_order":sorted(compatible),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"migration_order":sorted(migration),"semantic_loss_order":sorted(semantic_loss),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]}; digest=_hash(payload)
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"required_protocol":r["required_protocol"],"required_schema_version":r["required_schema_version"],"semantic_profile":r["semantic_profile"],"disposition":disposition,**payload,"interpretation_digest":digest,"artifact":{"artifact_id":f"scale-interpretation-interchange:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":["raw-data-local-and-aggregate-only"],"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:permitted-artifacts:{r['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; validate_interpretation_interoperability(out); return out
def interoperate_interpretations_json(value: Mapping[str,Any]) -> dict[str,Any]: return interoperate_interpretations(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","interpretation_interoperability_gateway_manifest","interoperate_interpretations","interoperate_interpretations_json","validate_interpretation_interoperability"]
