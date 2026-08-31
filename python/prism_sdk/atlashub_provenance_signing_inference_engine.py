"""Parity implementation for ``AFA-atlashub-P18-F01``."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-atlashub-P18-F01";CONTRACT_VERSION="atlashub-local-single-study-provenance-signing-inference-engine/1.0";INPUT_SCHEMA="ArtifactAndDerivation1@1";OUTPUT_SCHEMA="SignedProvenanceEnvelope1@1";CONTENT_TYPE="application/vnd.aurora.atlashub-signed-provenance-envelope-1+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def provenance_signing_inference_engine_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"atlashub","consumers":["integration engineer","provenance steward","research-object publisher"],"behavior":"compile local preclinical artifact lineage into a deterministic signer-bound provenance envelope with omission and negative-evidence witnesses","value":"lets integrations prove research-object lineage and replay identity without moving raw artifacts or treating missing provenance as valid","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def _validate(r:Mapping[str,Any])->None:
    if r.get("schema_version")!=INPUT_SCHEMA or any(not isinstance(r.get(k),str) or not r[k].strip() for k in ("request_id","consumer","purpose","signer_id")) or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or not r.get("artifacts"):raise ResearchContractError("request identity, replay, boundary, or artifact closure is invalid")
    ids:set[str]=set()
    for a in r["artifacts"]:
        if not isinstance(a.get("artifact_id"),str) or not a["artifact_id"].strip() or a["artifact_id"] in ids or not _ordered(a.get("derivation_order",[])) or not _ordered(a.get("source_order",[])) or not _digest(a.get("content_digest")) or not _digest(a.get("provenance_digest")) or a.get("replay_identity")!=r["replay_identity"]:raise ResearchContractError("artifact identity, ordering, digest, or replay is invalid")
        ids.add(a["artifact_id"])
def validate_signed_provenance(output:Mapping[str,Any])->None:
    a=output.get("artifact",{})
    if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("artifact_order") or not output.get("signer_id"):raise ResearchContractError("envelope identity, locality, disposition, or artifacts are incomplete")
    if any(not _ordered(output.get(k,[])) for k in ("artifact_order","signed_order","unresolved_order","blocked_order","omission_order","negative_evidence_order")):raise ResearchContractError("envelope ordering is not canonical")
    ids=set(output["artifact_order"]);parts=output.get("signed_order",[])+output.get("unresolved_order",[])+output.get("blocked_order",[])
    if len(ids)!=len(output["artifact_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("artifact states do not partition")
    if any(not _digest(output.get(k)) for k in ("replay_identity","signature_digest","envelope_digest")) or a.get("content_hash")!=output.get("envelope_digest") or any(not _digest(v) for v in a.get("provenance_digests",[])):raise ResearchContractError("envelope digest is inconsistent")
def infer_signed_provenance(r:Mapping[str,Any])->dict[str,Any]:
    _validate(r);rows=sorted((dict(a) for a in r["artifacts"]),key=lambda a:a["artifact_id"]);order=[a["artifact_id"] for a in rows];signed:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();omissions:set[str]=set();negative={a["artifact_id"] for a in rows if a.get("negative_result")}
    for a in rows:
        i=a["artifact_id"]
        if not a.get("local_only"):blocked.add(i);omissions.add(f"{i}:raw-data-not-local")
        elif a.get("evidence_state") not in {"proven","supported"}:unresolved.add(i);omissions.add(f"{i}:evidence-state")
        else:signed.add(i)
    for ok,label in ((r.get("policy_allowed"),"request:policy-denied"),(r.get("protected_closure"),"request:protected-closure-incomplete"),(r.get("raw_data_local"),"request:raw-data-not-local")):
        if not ok:omissions.add(label)
    global_block=not all(r.get(k) is True for k in ("policy_allowed","protected_closure","raw_data_local"));disposition="blocked" if global_block or blocked else "partial" if unresolved or not signed else "qualified"
    if global_block:blocked.update(order);signed.clear();unresolved.clear()
    if disposition!="qualified":omissions.add("request:provenance-closure-not-ready")
    payload={"artifact_order":order,"signed_order":sorted(signed),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"omission_order":sorted(omissions),"negative_evidence_order":sorted(negative),"replay_identity":r["replay_identity"]};ed=_hash(payload);sd=_hash({"signer_id":r["signer_id"],"envelope_digest":ed})
    out={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"consumer":r["consumer"],"purpose":r["purpose"],"signer_id":r["signer_id"],"disposition":disposition,**payload,"signature_digest":sd,"envelope_digest":ed,"artifact":{"artifact_id":f"atlashub-provenance:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":ed,"semantic_loss":[] if disposition=="qualified" else ["provenance-not-signed-for-release"],"provenance_digests":sorted({a["provenance_digest"] for a in rows}),"boundary":PRECLINICAL_BOUNDARY},"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY};validate_signed_provenance(out);return out
def infer_signed_provenance_json(value:Mapping[str,Any])->dict[str,Any]:return infer_signed_provenance(value)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","provenance_signing_inference_engine_manifest","infer_signed_provenance","infer_signed_provenance_json","validate_signed_provenance"]
