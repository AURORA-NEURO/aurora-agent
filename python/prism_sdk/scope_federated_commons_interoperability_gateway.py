"""Python parity for ``AFA-scope-P31-F24`` scope interoperability gateway."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-scope-P31-F24"; CONTRACT_VERSION="scope-federated-continual-scope-interoperability-gateway/1.0"; INPUT_SCHEMA="ScopeFederationGatewayRequest7@1"; OUTPUT_SCHEMA="ScopeFederationGatewayReceipt10@1"; CONTENT_TYPE="application/vnd.aurora.scope-federation-gateway-receipt-10+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class ScopeFederationGatewayReceipt10:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("disposition") not in {"qualified","partial","blocked"} or int(v.get("checkpoint_seq",0))<=0 or not v.get("candidate_order") or not v.get("peer_order") or not v.get("effect_receipts") or not all(str(v.get(k,"")).strip() for k in ("request_id","consumer","purpose","source_scope","target_scope","semantic_profile","capability_id","required_schema")):raise ResearchContractError("scope gateway identity, bounds, locality, candidates, peers, or effects are incomplete")
        for k in ("candidate_order","compatible_order","unresolved_order","blocked_order","missing_order","migration_order","omission_order","uncertainty_order","negative_evidence_order","peer_order","qualified_peer_order","missing_peer_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("scope gateway ordering is not canonical")
        ids=set(v["candidate_order"]);parts=[*v["compatible_order"],*v["unresolved_order"],*v["blocked_order"],*v["missing_order"]];peers=set(v["peer_order"]);pp=[*v["qualified_peer_order"],*v["missing_peer_order"]]
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or set(parts)!=ids or len(peers)!=len(v["peer_order"]) or len(pp)!=len(peers) or set(pp)!=peers:raise ResearchContractError("scope gateway candidate or peer states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","checkpoint_digest","gateway_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("gateway_digest") or not all(_digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("scope gateway digest or artifact metadata is invalid")
        if any(e!="block:unsafe-release" and not e.startswith("exchange:scope-summary:") for e in v["effect_receipts"]):raise ResearchContractError("scope gateway effect is outside exchange gate")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"exchange:scope-summary:{v['request_id']}"]:raise ResearchContractError("qualified scope gateway effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified scope gateway must block")
def federated_scope_interoperability_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"scope","consumers":["federation interoperability steward","scope migration operator","downstream research workflow"],"behavior":"negotiate continual federated scope and schema compatibility from signed digest-only peer manifests","value":"prevents incomparable scope summaries, unauthorized exports, and silent semantic loss at a federation boundary","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact","exchange:permitted-aggregates"],"permissions":["read:local-research-summaries","exchange:permitted-aggregates"],"determinism":"byte_stable","autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}
def operate_federated_scope_interoperability_gateway(request:Mapping[str,Any])->ScopeFederationGatewayReceipt10:
    if not all(str(request.get(k,"")).strip() for k in ("request_id","consumer","purpose","source_scope","target_scope","semantic_profile","capability_id","required_schema")) or int(request.get("checkpoint_seq",0))<=0 or not _digest(request.get("replay_identity")) or request.get("aggregate_only") is not True or request.get("raw_data_local") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY or not request.get("artifacts") or not request.get("peers"):raise ResearchContractError("scope gateway identity, manifests, replay, locality, or boundary are invalid")
    artifacts=[dict(x) for x in request["artifacts"]];candidates=sorted({str(x.get("artifact_id","")) for x in artifacts});
    if len(candidates)!=len(artifacts) or any(not x.strip() for x in candidates):raise ResearchContractError("artifact ids must be unique and non-empty")
    q=set();u=set();b=set();m=set();om=set();unc=set();neg=set()
    for x in artifacts:
        eid=str(x.get("artifact_id"));
        if x.get("negative_result"):neg.add(eid)
        valid=x.get("available") is True and x.get("permitted") is True and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("scope")==request["source_scope"] and x.get("semantic_profile")==request["semantic_profile"] and _digest(x.get("content_digest")) and _digest(x.get("provenance_digest"))
        if not x.get("available"):u.add(eid);om.add(f"artifact:{eid}:unavailable")
        elif not valid:b.add(eid);om.add(f"artifact:{eid}:policy-or-integrity")
        else:q.add(eid)
    peers=sorted({str(x.get("peer_id","")) for x in request["peers"]});
    if len(peers)!=len(request["peers"]) or any(not x.strip() for x in peers):raise ResearchContractError("peer ids must be unique and non-empty")
    qp=sorted({str(p.get("peer_id")) for p in request["peers"] if p.get("signed") is True and p.get("policy_allowed") is True and p.get("aggregate_only") is True and p.get("raw_data_local") is True and p.get("capability_id")==request["capability_id"] and p.get("schema")==request["required_schema"] and p.get("semantic_profile")==request["semantic_profile"] and p.get("scope")==request["target_scope"] and int(p.get("checkpoint_seq",0))==int(request["checkpoint_seq"])})
    mp=sorted(set(peers)-set(qp));
    if mp:om.add("peer-missing:"+",".join(mp));unc.add("peer-compatibility-incomplete")
    for k,label in (("policy_allow","workflow:policy-denied"),("protected_closure","workflow:protected-closure-incomplete"),("federation_approved","workflow:federation-approval-missing"),("signed_approval","workflow:signed-approval-missing")):
        if request.get(k) is not True:om.add(label)
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","federation_approved","signed_approval"));disp="blocked" if global_block or b else "partial" if not q or u or mp else "qualified";om.add("workflow:closure-incomplete") if disp!="qualified" else None
    if global_block:b.update(candidates);q.clear();u.clear()
    checkpoint=_hash({"request_id":request["request_id"],"checkpoint_seq":int(request["checkpoint_seq"]),"source_scope":request["source_scope"],"target_scope":request["target_scope"],"replay_identity":request["replay_identity"]});payload={"candidate_order":candidates,"compatible_order":sorted(q),"unresolved_order":sorted(u),"blocked_order":sorted(b),"missing_order":[],"migration_order":sorted(m),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"peer_order":peers,"qualified_peer_order":qp,"missing_peer_order":mp,"checkpoint_digest":checkpoint,"replay_identity":request["replay_identity"]};gateway=_hash(payload);payload.update({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"source_scope":request["source_scope"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"capability_id":request["capability_id"],"required_schema":request["required_schema"],"checkpoint_seq":int(request["checkpoint_seq"]),"disposition":disp,"gateway_digest":gateway,"artifact":{"artifact_id":f"scope-federation-gateway:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":gateway,"semantic_loss":[] if disp=="qualified" else ["scope-exchange-not-qualified"],"provenance_digests":sorted({str(x.get("provenance_digest")) for x in artifacts}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:scope-summary:{request['request_id']}"] if disp=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY});r=ScopeFederationGatewayReceipt10(payload);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ScopeFederationGatewayReceipt10","federated_scope_interoperability_manifest","operate_federated_scope_interoperability_gateway"]
