"""Cross-language contract for ``AFA-bioworlds-P05-F12``."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-bioworlds-P05-F12"
CONTRACT_VERSION = "bioworlds-federated-continual-resource-discovery-research-copilot/1.0"
INPUT_SCHEMA = "ResourceNeed5@1"
OUTPUT_SCHEMA = "QualifiedResourceSet6@1"
CONTENT_TYPE = "application/vnd.aurora.bioworlds-qualified-resource-set-6+json"

def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _ordered(values: list[str]) -> bool: return values == sorted(set(values))
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _partition(value: Mapping[str, Any], universe: str, parts: tuple[str, ...], message: str) -> None:
    all_values=list(value.get(universe, [])); flat=sum((list(value.get(part, [])) for part in parts), [])
    if len(all_values)!=len(set(all_values)) or len(flat)!=len(set(flat)) or set(flat)!=set(all_values): raise ResearchContractError(message)

@dataclass(frozen=True)
class QualifiedResourceSet6:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        v=self.value; artifact=v.get("artifact", {})
        if not (v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==CONTRACT_VERSION and v.get("feature_id")==FEATURE_ID and v.get("boundary")==PRECLINICAL_BOUNDARY and artifact.get("boundary")==PRECLINICAL_BOUNDARY and artifact.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("autonomy_tier")=="a2" and all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","requester","purpose","semantic_profile")) and v.get("ranked_resource_order") and v.get("capability_order") and v.get("site_order") and v.get("reasons") and v.get("effect_receipts") and v.get("disposition") in {"qualified","unresolved","blocked"}): raise ResearchContractError("resource identity, closure, locality, autonomy, or effects are incomplete")
        for field in ("ranked_resource_order","selected_resource_order","unresolved_resource_order","blocked_resource_order","missing_resource_order","capability_order","selected_capability_order","unresolved_capability_order","blocked_capability_order","missing_capability_order","site_order","selected_site_order","unresolved_site_order","blocked_site_order","missing_site_order","omission_order","uncertainty_order","negative_evidence_order","contradiction_order","adversarial_event_order"):
            if not _ordered(list(v.get(field, []))): raise ResearchContractError("resource receipt ordering is not canonical")
        _partition(v,"ranked_resource_order",("selected_resource_order","unresolved_resource_order","blocked_resource_order","missing_resource_order"),"resource states do not form a complete partition")
        _partition(v,"capability_order",("selected_capability_order","unresolved_capability_order","blocked_capability_order","missing_capability_order"),"capability states do not form a complete partition")
        _partition(v,"site_order",("selected_site_order","unresolved_site_order","blocked_site_order","missing_site_order"),"site states do not form a complete partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","provenance_digest","resource_digest")) or artifact.get("content_hash")!=v.get("resource_digest"): raise ResearchContractError("resource digest is inconsistent")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"read:authorized-resource-state:{v['request_id']}"]: raise ResearchContractError("qualified resource effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]: raise ResearchContractError("non-qualified resource discovery must block")
    def digest(self) -> str: self.validate(); return _hash(self.value)

def resource_discovery_manifest() -> dict[str, Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"bioworlds","consumers":["resource researcher","workbench operator","federation verifier"],"behavior":"ranks declared institution-local resource capabilities and emits an omission-aware qualified resource set without fetching resources","value":"makes multi-site resource discovery reproducible while preventing unknown or unauthorized capabilities from appearing available","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute_local_computation","federation_export"],"permissions":["read:authorized-resource-state"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}

def _validate_request(request: Mapping[str, Any]) -> None:
    if not (request.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and all(isinstance(request.get(k),str) and request[k].strip() for k in ("request_id","requester","purpose","semantic_profile")) and all(request.get(k) for k in ("required_resource_order","required_capability_order","required_site_order")) and all(_ordered(list(request[k])) for k in ("required_resource_order","required_capability_order","required_site_order")) and _ordered(list(request.get("adversarial_event_order",[]))) and request.get("minimum_resource_count",0)>0 and request.get("minimum_site_count",0)>0 and _digest(request.get("replay_identity")) and request.get("boundary")==PRECLINICAL_BOUNDARY and request.get("candidates")): raise ResearchContractError("resource identity, closure, bounds, replay, boundary, or candidates are invalid")
    seen=set()
    for row in request["candidates"]:
        if not (all(isinstance(row.get(k),str) and row[k].strip() for k in ("resource_id","capability_id","site_id","institution_id","semantic_profile")) and row["semantic_profile"]==request["semantic_profile"] and 0<=row.get("availability_milli",-1)<=1000 and 0<=row.get("trust_milli",-1)<=1000 and all(_digest(row.get(k)) for k in ("provenance_digest","replay_identity")) and _ordered(list(row.get("omission_order",[]))) and _ordered(list(row.get("uncertainty_order",[]))) and row["resource_id"] not in seen): raise ResearchContractError("resource candidate identity, profile, ranges, digest, or ordering is invalid")
        seen.add(row["resource_id"])

def qualify_resources(request: Mapping[str, Any]) -> QualifiedResourceSet6:
    _validate_request(request); rank={"proven":0,"supported":1,"speculative":2,"unknown":3,"contradicted":4}; rows=sorted((dict(x) for x in request["candidates"]),key=lambda x:(rank.get(x.get("evidence_state"),5),-x.get("trust_milli",0),-x.get("availability_milli",0),x["resource_id"]))
    ranked=[x["resource_id"] for x in rows]; required=set(request["required_resource_order"]); selected=set(); unresolved=set(); blocked=set(); omissions=set(); uncertainty=set(); negative=set(); contradiction=set()
    for row in rows:
        omissions.update(row.get("omission_order",[])); uncertainty.update(row.get("uncertainty_order",[]));
        if row.get("negative_result"): negative.add(row["resource_id"])
        if row.get("evidence_state")=="contradicted": contradiction.add(row["resource_id"])
        hard=row.get("revoked") is True or any(row.get(k) is not True for k in ("policy_allowed","federation_allowed","raw_data_local","aggregate_only")) or row.get("availability_milli",0)==0
        soft=row.get("stale") is True or row.get("replay_identity")!=request["replay_identity"] or row.get("availability_milli",0)<500 or row.get("trust_milli",0)<500 or row.get("omission_order") or row.get("uncertainty_order") or row.get("evidence_state") in {"unknown","speculative"}
        (blocked if hard or row.get("evidence_state")=="contradicted" else unresolved if soft else selected).add(row["resource_id"])
    missing=required-set(ranked); omissions.update(f"missing required resource: {x}" for x in missing); resources={x["resource_id"]:x for x in rows}; capabilities=set(request["required_capability_order"])|{x["capability_id"] for x in rows}; sites=set(request["required_site_order"])|{x["site_id"] for x in rows}
    def groups(key:str, universe:set[str]):
        chosen={x[key] for x in rows if x["resource_id"] in selected}; uncertain={x[key] for x in rows if x["resource_id"] in unresolved}-chosen; denied={x[key] for x in rows if x["resource_id"] in blocked}-chosen-uncertain; absent=universe-chosen-uncertain-denied; return chosen,uncertain,denied,absent
    selected_capabilities,unresolved_capabilities,blocked_capabilities,missing_capabilities=groups("capability_id",capabilities); selected_sites,unresolved_sites,blocked_sites,missing_sites=groups("site_id",sites)
    open_gate=all(request.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_allow","raw_data_local","aggregate_only")) and not request.get("adversarial_event_order"); disposition="blocked" if (not open_gate or blocked or missing or blocked_capabilities or missing_capabilities or blocked_sites or missing_sites or len(selected)<request["minimum_resource_count"] or len(selected_sites)<request["minimum_site_count"]) else "unresolved" if unresolved or unresolved_capabilities or unresolved_sites else "qualified"; effects=[f"read:authorized-resource-state:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"requester":request["requester"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"disposition":disposition,"ranked_resource_order":ranked,"selected_resource_order":sorted(selected),"unresolved_resource_order":sorted(unresolved),"blocked_resource_order":sorted(blocked),"missing_resource_order":sorted(missing),"capability_order":sorted(capabilities),"selected_capability_order":sorted(selected_capabilities),"unresolved_capability_order":sorted(unresolved_capabilities),"blocked_capability_order":sorted(blocked_capabilities),"missing_capability_order":sorted(missing_capabilities),"site_order":sorted(sites),"selected_site_order":sorted(selected_sites),"unresolved_site_order":sorted(unresolved_sites),"blocked_site_order":sorted(blocked_sites),"missing_site_order":sorted(missing_sites),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"contradiction_order":sorted(contradiction),"adversarial_event_order":sorted(request.get("adversarial_event_order",[])),"replay_identity":request["replay_identity"],"provenance_digest":_hash(sorted(x["provenance_digest"] for x in rows)),"reasons":["all resource, capability, site, policy, replay, provenance, and locality gates passed" if disposition=="qualified" else "resource admission remains blocked or unresolved"],"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"autonomy_tier":"a2","boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload); payload["resource_digest"]=digest; payload["artifact"]={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"qualified-resource-set:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY}; receipt=QualifiedResourceSet6(payload); receipt.validate(); return receipt

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","QualifiedResourceSet6","resource_discovery_manifest","qualify_resources"]
