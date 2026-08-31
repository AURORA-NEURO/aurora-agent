"""Python parity for the WeaveLang federated-commons assurance harness."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-weavelang-P31-F27"
CONTRACT_VERSION="weavelang-prospective-high-throughput-federated-commons-assurance-harness/1.0"
INPUT_SCHEMA="WeavelangFederationRequest5@1"
OUTPUT_SCHEMA="WeavelangFederationEnvelope8@1"
CONTENT_TYPE="application/vnd.aurora.weavelang-federation-envelope-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))

@dataclass(frozen=True)
class WeavelangFederationEnvelope8:
    value:Mapping[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value
        if (v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("artifact",{}).get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or any(not str(v.get(k,"")).strip() for k in ("request_id","federation_id","requester","purpose","semantic_profile")) or not v.get("capability_order") or not v.get("provider_order") or not v.get("effect_receipts")):
            raise ResearchContractError("WeaveLang federation identity, axes, locality, or effects are incomplete")
        keys=("capability_order","selected_capability_order","unresolved_capability_order","blocked_capability_order","missing_capability_order","provider_order","selected_provider_order","missing_provider_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in keys):raise ResearchContractError("WeaveLang federation ordering is not canonical")
        ids=set(v["capability_order"]);parts=v.get("selected_capability_order",[])+v.get("unresolved_capability_order",[])+v.get("blocked_capability_order",[])+v.get("missing_capability_order",[])
        if len(v["capability_order"])!=len(ids) or set(parts)!=ids or len(parts)!=len(set(parts)):raise ResearchContractError("WeaveLang capability states do not partition")
        if any(x not in ids for x in v.get("missing_capability_order",[])) or any(x not in set(v["provider_order"]) for x in v.get("missing_provider_order",[])):raise ResearchContractError("WeaveLang missing state is outside declared axes")
        a=v.get("artifact",{})
        if not all(_digest(x) for x in (v.get("replay_identity"),v.get("federation_digest"),a.get("content_hash"))) or v.get("federation_digest")!=a.get("content_hash") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("WeaveLang federation digest is invalid")
        if any(e!="block:unsafe-release" and not e.startswith("verify:weavelang-federation:") for e in v["effect_receipts"]):raise ResearchContractError("WeaveLang federation effect is outside governed gate")
        if v.get("disposition")=="qualified" and v["effect_receipts"]!=[f"verify:weavelang-federation:{v['request_id']}"]:raise ResearchContractError("qualified WeaveLang federation effect is invalid")
        if v.get("disposition")!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified WeaveLang federation must block")
    def digest(self)->str:self.validate();return _hash(self.value)

def assure_weavelang_federated_commons(*,request:Mapping[str,Any])->WeavelangFederationEnvelope8:
    if (request.get("schema_version")!=INPUT_SCHEMA or any(not str(request.get(k,"")).strip() for k in ("request_id","federation_id","requester","purpose","semantic_profile")) or not request.get("required_capability_order") or not request.get("required_provider_order") or not request.get("capabilities") or not _ordered(request["required_capability_order"]) or not _ordered(request["required_provider_order"]) or not _ordered(request.get("adversarial_events",[])) or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY):raise ResearchContractError("WeaveLang federation request identity, closure, replay, locality, or boundary is invalid")
    rows=sorted(request["capabilities"],key=lambda c:(str(c.get("provider_id","")),str(c.get("capability_id",""))))
    for c in rows:
        if (not str(c.get("capability_id","")).strip() or not str(c.get("provider_id","")).strip() or str(c.get("semantic_profile"))=="" or not all(_digest(c.get(k)) for k in ("artifact_digest","evidence_digest","provenance_digest","replay_identity")) or not _ordered(c.get("omission_order",[]))):raise ResearchContractError("WeaveLang capability identity, digests, or ordering are invalid")
    ids=sorted(set(request["required_capability_order"])|{str(c["capability_id"]) for c in rows}); providers=sorted(set(request["required_provider_order"])|{str(c["provider_id"]) for c in rows})
    selected,unresolved,blocked,missing,omission,uncertainty,negative=set(),set(),set(),set(),set(),set(),set()
    for c in rows:
        cid=str(c["capability_id"]);omission.update(f"{cid}:{x}" for x in c.get("omission_order",[]));negative.update({f"{cid}:negative-result"} if c.get("negative_result") else set())
        if str(c.get("semantic_profile"))!=str(request["semantic_profile"]):unresolved.add(cid);uncertainty.add(f"{cid}:semantic-profile")
        elif c.get("local_only") is not True or c.get("aggregate_only") is not True or c.get("policy_allow") is not True:blocked.add(cid);omission.add(f"{cid}:locality-or-policy")
        elif str(c.get("replay_identity"))!=str(request["replay_identity"]) or c.get("signed") is not True or c.get("protected_closure") is not True or str(c.get("evidence_state","")).lower() not in {"proven","supported"}:unresolved.add(cid)
        else:selected.add(cid)
    for cid in request["required_capability_order"]:
        if not any(str(c["capability_id"])==cid for c in rows):missing.add(cid);omission.add(f"capability:{cid}:missing")
    missing_providers={pid for pid in request["required_provider_order"] if not any(str(c["provider_id"])==pid for c in rows)}
    for pid in missing_providers:omission.add(f"provider:{pid}:missing")
    uncertainty.update(f"adversarial:{x}" for x in request.get("adversarial_events",[]))
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","signed_approval","federation_authorized","raw_data_local","aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block:blocked.update(str(c["capability_id"]) for c in rows);selected.clear();unresolved.clear();omission.add("request:federation-release-gate-blocked")
    disposition="blocked" if global_block else ("unresolved" if not selected or missing or missing_providers else "qualified")
    if disposition!="qualified":omission.add("request:federation-not-release-ready")
    selected_providers=sorted({str(c["provider_id"]) for c in rows if str(c["capability_id"]) in selected})
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":str(request["request_id"]),"federation_id":str(request["federation_id"]),"requester":str(request["requester"]),"purpose":str(request["purpose"]),"semantic_profile":str(request["semantic_profile"]),"disposition":disposition,"capability_order":ids,"selected_capability_order":sorted(selected),"unresolved_capability_order":sorted(unresolved),"blocked_capability_order":sorted(blocked),"missing_capability_order":sorted(missing),"provider_order":providers,"selected_provider_order":selected_providers,"missing_provider_order":sorted(missing_providers),"omission_order":sorted(omission),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"effect_receipts":[f"verify:weavelang-federation:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    d=_hash(payload);payload["replay_identity"]=str(request["replay_identity"]);payload["federation_digest"]=d;payload["artifact"]={"artifact_id":f"weavelang-federation:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(omission),"provenance_digests":sorted(str(c["provenance_digest"]) for c in rows),"boundary":PRECLINICAL_BOUNDARY}
    receipt=WeavelangFederationEnvelope8(payload);receipt.validate();return receipt
def weavelang_federated_commons_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"weavelang","consumers":["WeaveLang compiler steward","federation verifier","research automation operator"],"input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"autonomy_tier":"A1","effects":["verify:weavelang-federation","block:unsafe-release"],"boundary":PRECLINICAL_BOUNDARY}
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","WeavelangFederationEnvelope8","assure_weavelang_federated_commons","weavelang_federated_commons_assurance_manifest"]
