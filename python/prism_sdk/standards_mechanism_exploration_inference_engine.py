"""Python parity for ``AFA-standards-P08-F04`` mechanism assurance."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
FEATURE_ID="AFA-standards-P08-F04"; CONTRACT_VERSION="standards-federated-continual-mechanism-exploration-inference-engine/1.0"; INPUT_SCHEMA="StandardsMechanismQuestion6@1"; OUTPUT_SCHEMA="StandardsMechanismInferenceReceipt8@1"; CONTENT_TYPE="application/vnd.aurora.standards-mechanism-inference-receipt-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class StandardsMechanismInferenceReceipt8:
 value:dict[str,Any]
 def to_dict(self)->dict[str,Any]:return dict(self.value)
 def validate(self)->None:
  v=self.value;a=v.get("artifact",{}); parts=v.get("selected_order",[])+v.get("competing_order",[])+v.get("unresolved_order",[])+v.get("blocked_order",[])+v.get("missing_candidate_order",[])
  if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("checkpoint",0)<=0 or v.get("disposition") not in {"qualified","unresolved","blocked"} or not v.get("candidate_order") or not v.get("peer_order") or not v.get("effect_receipts"):raise ResearchContractError("mechanism identity, checkpoint, locality, candidates, peers, or effects are incomplete")
  fields=("candidate_order","ranked_order","selected_order","competing_order","unresolved_order","blocked_order","missing_candidate_order","missing_study_order","missing_modality_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","contradiction_order","negative_evidence_order","effect_receipts")
  if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("mechanism ordering is not canonical")
  if set(v["candidate_order"])!=set(parts) or len(set(v["candidate_order"]))!=len(v["candidate_order"]):raise ResearchContractError("candidate states do not partition")
  if len(v["ranked_order"])!=len(v["candidate_order"]) or set(v["ranked_order"])!=set(v["candidate_order"]):raise ResearchContractError("ranking is not a candidate permutation")
  if set(v["peer_order"])!=set(v["qualified_peer_order"])|set(v["missing_peer_order"]):raise ResearchContractError("peer states do not partition")
  if a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("assurance_digest") or not all(_digest(x) for x in [v.get("replay_identity"),v.get("assurance_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]):raise ResearchContractError("mechanism artifact digest is invalid")
def standards_mechanism_exploration_inference_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"standards","consumers":["mechanism scientist","adaptive-panel operator","federation steward"],"behavior":"ranks typed mechanism candidates and peer attestations under reproducibility and policy gates","value":"exposes competing explanations and missing evidence before an adaptive research decision","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["retain:standards-mechanism-inference","exchange:aggregate-mechanism-summary"],"permissions":["retain:mechanism-evidence","exchange:aggregate-mechanism"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def infer_standards_mechanisms(q:Mapping[str,Any])->StandardsMechanismInferenceReceipt8:
 if not all(str(q.get(k,"")).strip() for k in ("request_id","federation_id","researcher","purpose","semantic_profile")) or not q.get("required_study_order") or not q.get("required_modality_order") or not q.get("candidates") or not q.get("peers") or int(q.get("checkpoint",0))<=0 or int(q.get("minimum_peer_quorum",0))<=0 or q.get("boundary")!=PRECLINICAL_BOUNDARY or q.get("raw_data_local") is not True or q.get("aggregate_only") is not True or not _digest(q.get("replay_identity")):raise ResearchContractError("mechanism identity, bounds, candidates, peers, replay, locality, or boundary is invalid")
 rows=sorted((dict(x) for x in q["candidates"]),key=lambda x:(-int(x.get("support_milli",0)),-int(x.get("novelty_milli",0)),str(x.get("candidate_id",""))));ids=[str(x.get("candidate_id","")) for x in rows]
 if len(set(ids))!=len(ids) or any(not x.get("candidate_id") or not x.get("mechanism_id") or not x.get("study_id") or not x.get("modality") or x.get("semantic_profile")!=q["semantic_profile"] or not all(_digest(x.get(k)) for k in ("artifact_digest","provenance_digest","replay_identity")) or x.get("replay_identity")!=q["replay_identity"] or x.get("local_data") is not True for x in rows):raise ResearchContractError("candidate identity, profile, digests, replay, or locality is invalid")
 selected=set();competing=set();unresolved=set();blocked=set();studies={x["study_id"] for x in rows};mods={x["modality"] for x in rows};omissions=set();uncertainty=set();contradiction=set();negative=set()
 for x in rows:
  i=x["candidate_id"];s=x.get("evidence_state")
  if x.get("negative_result"):negative.add(f"{i}:negative-result")
  if s=="contradicted":blocked.add(i);contradiction.add(f"{i}:contradicted")
  elif s in {"unknown","speculative"}:unresolved.add(i);uncertainty.add(f"{i}:evidence-state")
  elif s in {"proven","supported"} and int(x.get("support_milli",0))>=int(q["support_threshold_milli"]) and x.get("independent_source") is True and x.get("local_data") is True and x.get("policy_allowed") is True:
   (selected if not selected else competing).add(i)
  else:unresolved.add(i);uncertainty.add(f"{i}:closure-or-threshold")
 missing_study=set(q["required_study_order"])-studies;missing_modality=set(q["required_modality_order"])-mods;omissions|={f"study:{x}:missing" for x in missing_study}|{f"modality:{x}:missing" for x in missing_modality}
 peers=sorted((dict(x) for x in q["peers"]),key=lambda x:str(x.get("peer_id","")));peer_ids=[str(x.get("peer_id","")) for x in peers];qualified={x["peer_id"] for x in peers if x.get("semantic_profile")==q["semantic_profile"] and int(x.get("checkpoint",0))==int(q["checkpoint"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven","supported"}};missing_peers=set(peer_ids)-qualified;uncertainty|={f"peer:{x}:not-qualified" for x in missing_peers};global_block=not all(q.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
 if q.get("policy_allow") is not True:negative.add("request:policy-denied")
 if q.get("protected_closure") is not True:uncertainty.add("request:protected-closure-incomplete")
 if q.get("signed_approval") is not True:uncertainty.add("request:signed-approval-missing")
 if q.get("federation_approved") is not True:uncertainty.add("request:federation-approval-missing")
 disposition="blocked" if global_block or blocked else "unresolved" if not selected or missing_study or missing_modality or unresolved or len(qualified)<int(q["minimum_peer_quorum"]) else "qualified";omissions.add("request:mechanism-not-release-ready") if disposition!="qualified" else None
 payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q["request_id"],"federation_id":q["federation_id"],"researcher":q["researcher"],"purpose":q["purpose"],"semantic_profile":q["semantic_profile"],"checkpoint":int(q["checkpoint"]),"disposition":disposition,"candidate_order":ids,"ranked_order":ids,"selected_order":sorted(selected),"competing_order":sorted(competing),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"missing_candidate_order":[],"missing_study_order":sorted(missing_study),"missing_modality_order":sorted(missing_modality),"peer_order":peer_ids,"qualified_peer_order":sorted(qualified),"missing_peer_order":sorted(missing_peers),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"contradiction_order":sorted(contradiction),"negative_evidence_order":sorted(negative),"replay_identity":q["replay_identity"],"boundary":PRECLINICAL_BOUNDARY};d=_hash(payload);result={**payload,"assurance_digest":d,"artifact":{"artifact_id":f"standards-mechanism-inference-receipt-8:{q['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"retain:standards-mechanism-inference:{q['request_id']}",f"exchange:aggregate-mechanism-summary:{q['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True};r=StandardsMechanismInferenceReceipt8(result);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","StandardsMechanismInferenceReceipt8","standards_mechanism_exploration_inference_manifest","infer_standards_mechanisms"]


