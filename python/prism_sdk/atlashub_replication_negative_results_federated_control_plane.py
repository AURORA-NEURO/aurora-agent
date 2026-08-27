"""Python parity for ``AFA-atlashub-P15-F29``.

Classifies independent replication summaries and retains null/negative evidence;
no raw measurements are read and no protocol is executed.
"""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-atlashub-P15-F29"; CONTRACT_VERSION="atlashub-local-single-study-replication-negative-results-federated-control-plane/1.0"; INPUT_SCHEMA="ClaimAndProtocol1@1"; OUTPUT_SCHEMA="ReplicationRecord8@1"; CONTENT_TYPE="application/vnd.aurora.replication-record-8+json"
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return tuple(values)==tuple(sorted(set(values)))
@dataclass(frozen=True)
class ReplicationRecord8:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k," ")).strip() for k in ("request_id","claim_id","protocol_id","semantic_profile")) or v.get("checkpoint",0)<=0 or not v.get("observation_order") or not v.get("site_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("replication identity, checkpoint, locality, observations, sites, peers, or effects are incomplete")
        fields=("observation_order","qualified_observation_order","unresolved_observation_order","blocked_observation_order","positive_order","null_order","negative_order","inconclusive_order","site_order","qualified_site_order","missing_site_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("replication ordering is not canonical")
        if set(v["observation_order"])!=set(v["qualified_observation_order"])|set(v["unresolved_observation_order"])|set(v["blocked_observation_order"]):raise ResearchContractError("replication observations do not partition")
        if set(v["site_order"])!=set(v["qualified_site_order"])|set(v["missing_site_order"]):raise ResearchContractError("replication sites do not partition")
        if set(v["peer_order"])!=set(v["qualified_peer_order"])|set(v["missing_peer_order"]):raise ResearchContractError("replication peers do not partition")
        a=v.get("artifact",{}); ds=[v.get("replay_identity"),v.get("record_digest"),a.get("content_hash"),*a.get("provenance_digests",[])];
        if not all(_digest(x) for x in ds) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("record_digest"):raise ResearchContractError("replication artifact or digest is invalid")
        if any(not e.startswith(("exchange:permitted-summaries:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("replication effect is outside governed gate")
def replication_control_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"atlashub","consumers":["integration engineer","replication scientist","federation steward"],"behavior":"classifies independent replication observations and negative results under typed protocol, provenance, replay, policy, and federation gates","value":"prevents null, negative, contradictory, or incomparable replication evidence from being hidden in a positive claim","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability","exchange:permitted-summaries"],"permissions":["operate:institution-node"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def operate_replication_control(request_id:str,claim:Mapping[str,Any],observations:Sequence[Mapping[str,Any]],peers:Sequence[Mapping[str,Any]])->ReplicationRecord8:
    if not all(str(claim.get(k,"")).strip() for k in ("claim_id","protocol_id","claim_text","semantic_profile","expected_direction")) or int(claim.get("minimum_replicates",0))<=0 or not all(_digest(claim.get(k)) for k in ("protocol_digest","baseline_digest","replay_identity")) or claim.get("boundary")!=PRECLINICAL_BOUNDARY or claim.get("raw_data_local") is not True or claim.get("aggregate_only") is not True or not observations or not peers:raise ResearchContractError("replication claim identity, digests, locality, observations, or peers are invalid")
    rows=sorted((dict(x) for x in observations),key=lambda x:(str(x.get("site_id","")),str(x.get("observation_id","")))); ids=[str(x.get("observation_id","")) for x in rows];
    if len(set(ids))!=len(ids) or any(not x.get("observation_id") or not x.get("site_id") or not x.get("origin") or not all(_digest(x.get(k)) for k in ("artifact_digest","provenance_digest","replay_identity")) for x in rows):raise ResearchContractError("replication observation identity or digest is invalid")
    ps=sorted((dict(x) for x in peers),key=lambda x:str(x.get("peer_id",""))); peer_ids=[str(x.get("peer_id","")) for x in ps];
    if len(set(peer_ids))!=len(peer_ids) or any(not x.get("peer_id") or not x.get("origin") or not _digest(x.get("report_digest")) for x in ps):raise ResearchContractError("replication peer identity or digest is invalid")
    q=set();u=set();b=set();pos=set();nul=set();neg=set();inc=set();om=set();unc=set();ne=set(); effects=[]; vals=[]
    for x in rows:
        oid=x["observation_id"]; outcome=x.get("outcome")
        om.update(f"{oid}:{r}" for r in x.get("omission_reasons",[]))
        if x.get("negative_result") or outcome in {"negative","null"}:ne.add(f"{oid}:negative-or-null")
        if outcome=="positive":pos.add(oid);vals.append(int(x.get("effect_milli",0)))
        elif outcome=="null":nul.add(oid)
        elif outcome=="negative":neg.add(oid)
        elif outcome=="inconclusive":inc.add(oid)
        compatible=x.get("protocol_id")==claim["protocol_id"] and x.get("semantic_profile")==claim["semantic_profile"] and x.get("replay_identity")==claim["replay_identity"] and x.get("signed") is True and x.get("comparable") is True and x.get("raw_data_local") is True and x.get("aggregate_only") is True
        if x.get("evidence_state")=="contradicted":b.add(oid);ne.add(f"{oid}:contradicted")
        elif not compatible or x.get("evidence_state") not in {"proven","supported"}:u.add(oid);unc.add(f"{oid}:unresolved")
        else:q.add(oid)
    qualified_peers={x["peer_id"] for x in ps if x.get("claim_id")==claim["claim_id"] and x.get("semantic_profile")==claim["semantic_profile"] and int(x.get("checkpoint",0))==int(claim["minimum_replicates"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven","supported"}}; missing_peers=set(peer_ids)-qualified_peers;unc.update(f"peer:{x}:not-qualified" for x in missing_peers)
    global_block=not all(claim.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"));
    if claim.get("policy_allow") is not True:ne.add("request:policy-denied")
    if claim.get("protected_closure") is not True:unc.add("request:protected-closure-incomplete")
    if claim.get("signed_approval") is not True:unc.add("request:signed-approval-missing")
    if claim.get("federation_approved") is not True:unc.add("request:federation-approval-missing")
    disposition="blocked" if global_block or b else "unresolved" if len(q)<int(claim["minimum_replicates"]) or not qualified_peers or neg or nul or inc else "qualified"; om.add("request:replication-gates-incomplete") if disposition!="qualified" else None
    if global_block:b.update(ids);q.clear();u.clear()
    qo=sorted(q);uo=sorted(u);bo=sorted(b);so=sorted({x["site_id"] for x in rows});qsites=sorted({x["site_id"] for x in rows if x["observation_id"] in q});msites=sorted(set(so)-set(qsites)); vals.sort(); med=vals[len(vals)//2] if vals else 0
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request_id,"claim_id":claim["claim_id"],"protocol_id":claim["protocol_id"],"semantic_profile":claim["semantic_profile"],"checkpoint":int(claim["minimum_replicates"]),"disposition":disposition,"observation_order":ids,"qualified_observation_order":qo,"unresolved_observation_order":uo,"blocked_observation_order":bo,"positive_order":sorted(pos),"null_order":sorted(nul),"negative_order":sorted(neg),"inconclusive_order":sorted(inc),"site_order":so,"qualified_site_order":qsites,"missing_site_order":msites,"peer_order":peer_ids,"qualified_peer_order":sorted(qualified_peers),"missing_peer_order":sorted(missing_peers),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(ne),"recovery_order":[],"effect_median_milli":med,"positive_count":len(pos),"null_count":len(nul),"negative_count":len(neg),"replay_identity":claim["replay_identity"],"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload); result={**payload,"record_digest":digest,"artifact":{"artifact_id":f"replication-record-8:{request_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:permitted-summaries:{request_id}",f"manage:local-capability:{request_id}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True}; receipt=ReplicationRecord8(result);receipt.validate();return receipt
def atlashubReplicationControlDigest(receipt:ReplicationRecord8)->str:receipt.validate();return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ReplicationRecord8","replication_control_manifest","operate_replication_control","atlashubReplicationControlDigest"]
