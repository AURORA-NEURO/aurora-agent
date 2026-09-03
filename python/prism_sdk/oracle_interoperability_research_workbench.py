"""Parity implementation for ``AFA-oracle-P22-F17``."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-oracle-P22-F17"; CONTRACT_VERSION="oracle-local-single-study-interoperability-research-workbench/1.0"; INPUT_SCHEMA="ExternalCapability1@1"; OUTPUT_SCHEMA="NegotiatedIntegration5@1"; CONTENT_TYPE="application/vnd.aurora.oracle-negotiated-integration-5+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def interoperability_research_workbench_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"oracle","consumers":["bioinformatician","extension steward","benchmark curator"],"behavior":"negotiate external research capability schemas and standards with deterministic compatibility, semantic-loss, and provenance witnesses","value":"gives bioinformaticians a portable workbench view of compatible extensions without invoking untrusted providers or hiding limitations","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["view:authorized-research-state"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def _validate(r:Mapping[str,Any])->None:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","target_capability")) or not _ordered(r.get("required_schema_order",[])) or not _ordered(r.get("required_standard_order",[])) or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or not r.get("capabilities"):raise ResearchContractError("request identity, ordering, replay, boundary, or capability closure is invalid")
    ids:set[str]=set()
    for c in r["capabilities"]:
        if not isinstance(c.get("capability_id"),str) or not c["capability_id"].strip() or c["capability_id"] in ids or not _ordered(c.get("schema_order",[])) or not _ordered(c.get("standard_order",[])) or not _ordered(c.get("semantic_loss_order",[])) or not _digest(c.get("artifact_digest")) or not _digest(c.get("provenance_digest")) or c.get("replay_identity")!=r["replay_identity"]:raise ResearchContractError("capability identity, ordering, digest, or replay is invalid")
        ids.add(c["capability_id"])
def validate_interoperability_workbench(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("capability_order"):raise ResearchContractError("integration identity, locality, disposition, or capability closure is incomplete")
    if any(not _ordered(output.get(k,[])) for k in ("capability_order","compatible_order","unresolved_order","blocked_order","semantic_loss_order","negative_evidence_order")):raise ResearchContractError("integration ordering is not canonical")
    ids=set(output["capability_order"]);parts=output.get("compatible_order",[])+output.get("unresolved_order",[])+output.get("blocked_order",[])
    if len(ids)!=len(output["capability_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("capability states do not partition")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("integration_digest")) or a.get("content_hash")!=output.get("integration_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])):raise ResearchContractError("integration digest is inconsistent")
def negotiate_integration(r:Mapping[str,Any])->dict[str,Any]:
    _validate(r); rows=sorted((dict(c) for c in r["capabilities"]),key=lambda c:c["capability_id"]);order=[c["capability_id"] for c in rows];compatible:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();loss:set[str]=set();negative:set[str]=set()
    for c in rows:
        i=c["capability_id"];loss.update(f"{i}:{x}" for x in c.get("semantic_loss_order",[]));
        if c.get("negative_result"):negative.add(i)
        schema=all(x in c.get("schema_order",[]) for x in r["required_schema_order"]); standard=all(x in c.get("standard_order",[]) for x in r["required_standard_order"])
        if not c.get("enabled") or not c.get("local_only") or not c.get("supported"):blocked.add(i);loss.add(f"{i}:disabled-unsupported-or-nonlocal")
        elif i!=r["target_capability"] or not schema or not standard:unresolved.add(i);loss.add(f"{i}:schema-or-standard-mismatch")
        elif c.get("evidence_state") in {"proven","supported"}:compatible.add(i)
        else:unresolved.add(i);loss.add(f"{i}:evidence-state")
    global_block=not all(r.get(k) is True for k in ("policy_allowed","protected_closure","raw_data_local"));disposition="blocked" if global_block or blocked else "partial" if unresolved or not compatible else "qualified"
    if global_block:blocked.update(order);compatible.clear();unresolved.clear();loss.add("request:policy-protected-closure-or-locality-blocked")
    if disposition!="qualified":loss.add("request:integration-closure-not-ready")
    payload={"capability_order":order,"compatible_order":sorted(compatible),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"semantic_loss_order":sorted(loss),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]};digest=_hash(payload)
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"target_capability":r["target_capability"],"disposition":disposition,**payload,"integration_digest":digest,"artifact":{"artifact_id":f"oracle-negotiated-integration:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":["provider-not-invoked"],"provenance_digests":sorted({c["provenance_digest"] for c in rows}),"boundary":PRECLINICAL_BOUNDARY},"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY};validate_interoperability_workbench(out);return out
def negotiate_integration_json(value:Mapping[str,Any])->dict[str,Any]:return negotiate_integration(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","interoperability_research_workbench_manifest","negotiate_integration","negotiate_integration_json","validate_interoperability_workbench"]
