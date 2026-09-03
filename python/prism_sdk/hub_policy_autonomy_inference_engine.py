"""Parity implementation for ``AFA-hub-P19-F03``."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-hub-P19-F03";CONTRACT_VERSION="hub-prospective-high-throughput-policy-autonomy-inference-engine/1.0";INPUT_SCHEMA="ActionAndAuthority3@1";OUTPUT_SCHEMA="PolicyReceipt1@1";CONTENT_TYPE="application/vnd.aurora.hub-policy-receipt-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def policy_autonomy_inference_engine_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"hub","consumers":["consortium administrator","policy steward","workflow operator"],"behavior":"classify prospective high-throughput research actions into auditable allow, approval, local-only, deny, or unresolved policy receipts","value":"prevents policy-bounded automation from mistaking missing authority or evidence for permission","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate(r:Mapping[str,Any])->None:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","required_scope","policy_epoch")) or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or not r.get("actions"):raise ResearchContractError("policy identity, replay, epoch, boundary, or action closure is invalid")
    ids:set[str]=set()
    for a in r["actions"]:
        if not isinstance(a.get("action_id"),str) or not a["action_id"].strip() or a["action_id"] in ids or not a.get("actor") or not a.get("autonomy_tier") or not a.get("scope") or not _ordered(a.get("requested_effect_order",[])) or not _digest(a.get("artifact_digest")) or not _digest(a.get("provenance_digest")) or a.get("replay_identity")!=r["replay_identity"]:raise ResearchContractError("action identity, effect ordering, digest, or replay is invalid")
        ids.add(a["action_id"])
def validate_policy_receipt(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("action_order"):raise ResearchContractError("policy receipt identity, locality, disposition, or actions are incomplete")
    fields=("action_order","allowed_order","approval_required_order","local_only_order","denied_order","unresolved_order","omission_order","uncertainty_order","negative_evidence_order")
    if any(not _ordered(output.get(k,[])) for k in fields):raise ResearchContractError("policy receipt ordering is not canonical")
    ids=set(output["action_order"]);parts=sum((output.get(k,[]) for k in ("allowed_order","approval_required_order","local_only_order","denied_order","unresolved_order")),[])
    if len(ids)!=len(output["action_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("policy action states do not partition")
    if any(not _digest(output.get(k)) for k in ("replay_identity","receipt_digest")) or a.get("content_hash")!=output.get("receipt_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])):raise ResearchContractError("policy receipt digest is inconsistent")
def infer_policy_receipt(r:Mapping[str,Any])->dict[str,Any]:
    _validate(r);rows=sorted((dict(a) for a in r["actions"]),key=lambda a:a["action_id"]);order=[a["action_id"] for a in rows];allowed:set[str]=set();approval:set[str]=set();local:set[str]=set();denied:set[str]=set();unresolved:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative={a["action_id"] for a in rows if a.get("negative_result")}
    for a in rows:
        i=a["action_id"]
        if a.get("scope")!=r["required_scope"] or not a.get("policy_allowed") or not a.get("authority_present"):denied.add(i);omissions.add(f"{i}:scope-policy-or-authority")
        elif a.get("approval_required"):approval.add(i)
        elif a.get("local_only"):local.add(i)
        elif a.get("evidence_state") in {"unknown","speculative","contradicted"}:unresolved.add(i);uncertainty.add(f"{i}:evidence-state")
        else:allowed.add(i)
    for ok,label in ((r.get("protected_closure"),"request:protected-closure-incomplete"),(r.get("raw_data_local"),"request:raw-data-not-local")):
        if not ok:omissions.add(label)
    global_block=not all(r.get(k) is True for k in ("protected_closure","raw_data_local"));disposition="blocked" if global_block or denied else "partial" if unresolved or approval or local or not allowed else "qualified"
    if global_block:denied.update(order);allowed.clear();approval.clear();local.clear();unresolved.clear()
    if disposition!="qualified":omissions.add("request:policy-closure-not-ready")
    payload={"action_order":order,"allowed_order":sorted(allowed),"approval_required_order":sorted(approval),"local_only_order":sorted(local),"denied_order":sorted(denied),"unresolved_order":sorted(unresolved),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]};d=_hash(payload)
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"required_scope":r["required_scope"],"policy_epoch":r["policy_epoch"],"disposition":disposition,**payload,"receipt_digest":d,"artifact":{"artifact_id":f"hub-policy-receipt:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":[] if disposition=="qualified" else ["action-not-executed"],"provenance_digests":sorted({a["provenance_digest"] for a in rows}),"boundary":PRECLINICAL_BOUNDARY},"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY};validate_policy_receipt(out);return out
def infer_policy_receipt_json(value:Mapping[str,Any])->dict[str,Any]:return infer_policy_receipt(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","policy_autonomy_inference_engine_manifest","infer_policy_receipt","infer_policy_receipt_json","validate_policy_receipt"]
