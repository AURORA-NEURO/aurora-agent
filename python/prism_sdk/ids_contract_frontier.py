"""Python parity for ``AFA-ids-P25-F27`` contract-frontier assurance."""
from __future__ import annotations
import hashlib,json,re
from dataclasses import dataclass
from typing import Any,Mapping
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-ids-P25-F27";CONTRACT_VERSION="ids-prospective-high-throughput-contract-frontier-assurance-harness/1.0";INPUT_SCHEMA="IdsContractInput8@1";OUTPUT_SCHEMA="IdsCapabilityManifest9@1";CONTENT_TYPE="application/vnd.aurora.ids-capability-manifest-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class IdsCapabilityManifest9:
 value:dict[str,Any]
 def to_dict(self)->dict[str,Any]:return dict(self.value)
 def validate(self)->None:
  v=self.value;a=v.get("artifact",{}); ids=set(v.get("contract_order",[])); parts=v.get("accepted_order",[])+v.get("migrated_order",[])+v.get("unresolved_order",[])+v.get("blocked_order",[])+v.get("incompatible_order",[])
  if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(v.get(k," ").strip() for k in ("request_id","purpose","semantic_profile","required_contract_family")) or not v.get("contract_order") or not v.get("effect_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("manifest identity, locality, contracts, effects, or disposition is incomplete")
  if any(not _ordered(v.get(k,[])) for k in ("contract_order","accepted_order","migrated_order","unresolved_order","blocked_order","missing_effect_order","incompatible_order","omission_order","uncertainty_order","negative_evidence_order","effect_order","effect_receipts")):raise ResearchContractError("manifest ordering is not canonical")
  if len(ids)!=len(v["contract_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("contract states do not partition")
  if not all(_digest(d) for d in [v.get("replay_identity"),v.get("manifest_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]) or a.get("content_hash")!=v.get("manifest_digest") or a.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("manifest digest or artifact metadata is inconsistent")
  if any(not e.startswith(("exchange:capability-manifests:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("manifest effect is outside governed gate")
def contract_frontier_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["contract steward","SDK integrator","federation operator","release auditor"],"behavior":"assure prospective IDS capability manifests with effect, schema, evidence, replay, and policy gates","value":"prevents semantic drift, unsafe effects, and incomplete contracts from entering high-throughput research federation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:capability-manifests","manage:local-capability"],"permissions":["read:local-contract-manifests","request:contract-frontier-assurance"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
 if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","purpose","semantic_profile","required_contract_family")) or not r.get("required_effects") or not r.get("inputs") or len(r["inputs"])>16384 or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("frontier identity, effects, input bound, replay, or locality is invalid")
 ids=set()
 for i in r["inputs"]:
  if not all(isinstance(i.get(k),str) and i[k].strip() for k in ("contract_id","contract_family","version")) or not _digest(i.get("interface_digest")) or not _digest(i.get("manifest_digest")) or not i.get("required_effects") or not i.get("permissions") or not _digest(i.get("provenance_digest")) or not _digest(i.get("replay_identity")) or i["contract_id"] in ids:raise ResearchContractError("contract identity, version, effects, permissions, digest, or uniqueness is invalid")
  ids.add(i["contract_id"])
def assure_contract_frontier(r:Mapping[str,Any])->IdsCapabilityManifest9:
 _validate_request(r);ins=sorted((dict(i) for i in r["inputs"]),key=lambda i:i["contract_id"]);order=[i["contract_id"] for i in ins];required=set(r["required_effects"]);accepted=set();migrated=set();unresolved=set();blocked=set();incompatible=set();missing=set();omissions=set();uncertainty=set();negative=set();effects=set();prov=set()
 for i in ins:
  x=i["contract_id"]
  if i["contract_family"]!=r["required_contract_family"]:incompatible.add(x);omissions.add(f"{x}:contract-family")
  elif i.get("local") is not True or i.get("aggregate_only") is not True:blocked.add(x);omissions.add(f"{x}:raw-data-locality")
  elif i["replay_identity"]!=r["replay_identity"]:unresolved.add(x);uncertainty.add(f"{x}:replay-identity")
  elif i.get("evidence_state") in {"contradicted"}:blocked.add(x);negative.add(f"{x}:contradicted")
  elif i.get("evidence_state") not in {"proven","supported"}:unresolved.add(x);uncertainty.add(f"{x}:evidence-state")
  else:
   offered=set(i["required_effects"]);missing.update(f"{x}:{e}" for e in required-offered)
   if required<=offered:accepted.add(x);effects.update(offered);prov.add(i["provenance_digest"])
   else:unresolved.add(x);omissions.add(f"{x}:missing-effect")
 global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
 if global_block:blocked.update(order);accepted.clear();migrated.clear();unresolved.clear();incompatible.clear();omissions.add("request:governance-or-locality-denied")
 ao,mo,uo,bo,io=(sorted(x) for x in (accepted,migrated,unresolved,blocked,incompatible));disp="blocked" if global_block or (not ao and not mo and not uo) else "unresolved" if uo or bo or io or missing else "qualified";omissions.add("request:contract-frontier-not-closed") if disp!="qualified" else None;effects.add("block:unsafe-release") if disp!="qualified" else None
 p={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"required_contract_family":r["required_contract_family"],"disposition":disp,"contract_order":order,"accepted_order":ao,"migrated_order":mo,"unresolved_order":uo,"blocked_order":bo,"missing_effect_order":sorted(missing),"incompatible_order":io,"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"effect_order":sorted(effects),"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(p);p["manifest_digest"]=d;p["artifact"]={"artifact_id":f"ids-capability-manifest-9:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(omissions),"provenance_digests":sorted(prov),"boundary":PRECLINICAL_BOUNDARY};p["effect_receipts"]=sorted([f"exchange:capability-manifests:{r['request_id']}",f"manage:local-capability:{r['request_id']}"] if disp=="qualified" else ["block:unsafe-release"]);out=IdsCapabilityManifest9(p);out.validate();return out
def idsContractFrontierDigest(manifest:IdsCapabilityManifest9)->str:manifest.validate();return _hash(manifest.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","IdsCapabilityManifest9","contract_frontier_manifest","assure_contract_frontier","idsContractFrontierDigest"]
