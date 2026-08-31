"""Parity surface for ``AFA-stress-P06-F08``."""
from __future__ import annotations
import hashlib,json,re
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-stress-P06-F08"; CONTRACT_VERSION="stress-federated-continual-multimodal-ingestion-contract-model/1.0"; INPUT_SCHEMA="RawModalityBundle4@1"; OUTPUT_SCHEMA="HarmonizedResearchObject2@1"; CONTENT_TYPE="application/vnd.aurora.harmonized-research-object-2+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _valid(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def federated_multimodal_ingestion_contract_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"stress","consumers":["preclinical neuroscientist","multimodal ingestion operator","federation steward"],"behavior":"validate multimodal modality manifests into a deterministic harmonized research-object contract without reading raw bytes","value":"exchanges comparable metadata while preserving semantic loss, omissions, and local-data boundaries","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["read:local-data","write:local-artifact"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def validate_harmonized_research_object(output:Mapping[str,Any])->None:
 a=output.get("artifact",{}); keys=("modality_order","qualified_modality_order","unresolved_modality_order","blocked_modality_order","missing_modality_order","semantic_loss_order","peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order")
 if output.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version")!=CONTRACT_VERSION or output.get("feature_id")!=FEATURE_ID or output.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified","partial","blocked"} or not output.get("modality_order"):raise ResearchContractError("harmonized identity, locality, disposition, or modalities are incomplete")
 if any(not _ordered(output.get(k,[])) for k in keys):raise ResearchContractError("harmonized ordering is not canonical")
 ids=set(output["modality_order"]); parts=output["qualified_modality_order"]+output["unresolved_modality_order"]+output["blocked_modality_order"]
 if len(ids)!=len(output["modality_order"]) or set(parts)!=ids:raise ResearchContractError("modality states do not partition")
 if not _valid(output.get("replay_identity")) or not _valid(output.get("object_digest")) or a.get("content_hash")!=output.get("object_digest"):raise ResearchContractError("harmonized digest is invalid")
def harmonize_federated_multimodal(request:Mapping[str,Any])->dict[str,Any]:
 if request.get("schema_version")!=INPUT_SCHEMA or not request.get("request_id") or not request.get("study_id") or not request.get("required_modalities") or not request.get("modalities") or not _ordered(request["required_modalities"]) or not _valid(request.get("replay_identity")) or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:raise ResearchContractError("identity, required modalities, peers, replay, locality, or boundary is invalid")
 mods=sorted((dict(m) for m in request["modalities"]),key=lambda m:m.get("modality_id","")); ids=[m["modality_id"] for m in mods]; qualified=set(); unresolved=set(); blocked=set(); missing=set(); loss=set(); omissions=set(); uncertainty=set(); negative=set()
 for m in mods:
  mid=m["modality_id"]
  if m.get("negative_result"):negative.add(f"{mid}:negative-result")
  if m.get("qc_digest") is None:loss.add(f"{mid}:qc-digest-missing")
  if m.get("evidence_state")=="contradicted":blocked.add(mid);loss.add(f"{mid}:contradicted")
  elif m.get("evidence_state") in {"unknown","speculative"} or m.get("qc_digest") is None:unresolved.add(mid);uncertainty.add(f"{mid}:quality-or-evidence-uncertain")
  else:qualified.add(mid)
 for req in request["required_modalities"]:
  if not any(req in (m.get("modality_id"),m.get("modality_type")) for m in mods):missing.add(req);omissions.add(f"request:missing-modality:{req}")
 missing_peer=set() if request.get("peer_order") else {"request:peer-quorum-missing"}; global_block=not all(request.get(k) is True for k in ("policy_allow","protected_closure","raw_data_local","aggregate_only"))
 if global_block:blocked.update(ids);qualified.clear();unresolved.clear();omissions.add("request:policy-protected-closure-or-locality-blocked")
 disp="blocked" if global_block or (blocked and not qualified) else "partial" if unresolved or missing or missing_peer or blocked else "qualified"; omissions.add("request:harmonization-closure-not-ready") if disp!="qualified" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"study_id":request["study_id"],"semantic_profile":request.get("semantic_profile",""),"disposition":disp,"modality_order":ids,"qualified_modality_order":sorted(qualified),"unresolved_modality_order":sorted(unresolved),"blocked_modality_order":sorted(blocked),"missing_modality_order":sorted(missing),"semantic_loss_order":sorted(loss),"peer_order":sorted(request.get("peer_order",[])),"missing_peer_order":sorted(missing_peer),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; d=_hash(payload);payload["object_digest"]=d;payload["artifact"]={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"stress-harmonized:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY};validate_harmonized_research_object(payload);return payload
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","federated_multimodal_ingestion_contract_manifest","harmonize_federated_multimodal","validate_harmonized_research_object"]
