"""Python parity for ``AFA-ids-P03-F04`` context compilation."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P03-F04"; CONTRACT_VERSION="ids-federated-continual-context-compilation-inference-engine/1.0"; INPUT_SCHEMA="DecisionQuery4@1"; OUTPUT_SCHEMA="CertifiedDecisionSection1@1"; CONTENT_TYPE="application/vnd.aurora.certified-decision-section-1+json"; MAX_FACTS=16384
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return tuple(values)==tuple(sorted(set(values)))

@dataclass(frozen=True)
class CertifiedDecisionSection1:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value; required=("request_id","federation_id","requester","purpose","semantic_profile")
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in required) or int(v.get("checkpoint",0))<=0 or not v.get("candidate_order") or not v.get("source_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}: raise ResearchContractError("context identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete")
        fields=("candidate_order","selected_order","unresolved_order","blocked_order","source_order","selected_source_order","missing_source_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields): raise ResearchContractError("context ordering is not canonical")
        if set(v["candidate_order"])!=set(v["selected_order"])|set(v["unresolved_order"])|set(v["blocked_order"]): raise ResearchContractError("candidate states do not partition")
        if set(v["source_order"])!=set(v["selected_source_order"])|set(v["missing_source_order"]): raise ResearchContractError("source states do not partition")
        if set(v["peer_order"])!=set(v["qualified_peer_order"])|set(v["missing_peer_order"]): raise ResearchContractError("peer states do not partition")
        a=v.get("artifact",{}); ds=[v.get("replay_identity"),v.get("section_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]
        if not all(_digest(x) for x in ds) or len(v["selected_order"])!=len(v.get("influence_scores_milli",[])) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("section_digest"): raise ResearchContractError("context artifact, digest, or score cardinality is invalid")
        if any(not e.startswith(("exchange:permitted-context-summaries:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]): raise ResearchContractError("context effect is outside governed gate")

def context_compilation_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["formal methods researcher","computational biologist","federation steward"],"behavior":"compiles bounded typed decision facts and peer context attestations into an omission-aware certified section","value":"makes federated context selection deterministic, replayable, and fail-closed without exporting raw research data","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:permitted-context-summaries","manage:local-capability"],"permissions":["read:local-research-artifacts","operate:institution-node"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}

def operate_context_compilation(request:Mapping[str,Any],facts:Sequence[Mapping[str,Any]],peers:Sequence[Mapping[str,Any]])->CertifiedDecisionSection1:
    if not all(str(request.get(k,"")).strip() for k in ("request_id","federation_id","requester","purpose","semantic_profile")) or not request.get("required_claims") or int(request.get("candidate_limit",0))<=0 or int(request.get("candidate_limit",0))>MAX_FACTS or int(request.get("minimum_source_quorum",0))<=0 or int(request.get("minimum_peer_quorum",0))<=0 or int(request.get("budget_units",0))<=0 or int(request.get("checkpoint",0))<=0 or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")) or not facts or not peers: raise ResearchContractError("context request identity, claims, bounds, replay, locality, facts, peers, or boundary is invalid")
    rows=sorted((dict(x) for x in facts),key=lambda x:(-int(x.get("influence_milli",0)),-int(x.get("freshness_milli",0)),str(x.get("fact_id","")))); ids=[str(x.get("fact_id","")) for x in rows]
    if len(set(ids))!=len(ids) or any(not x.get("fact_id") or not x.get("source_id") or not x.get("origin") or not x.get("claim") or not all(_digest(x.get(k)) for k in ("content_digest","provenance_digest","replay_identity")) for x in rows): raise ResearchContractError("fact identity, uniqueness, origin, claim, or digest is invalid")
    ps=sorted((dict(x) for x in peers),key=lambda x:str(x.get("peer_id",""))); peer_ids=[str(x.get("peer_id","")) for x in ps]
    if len(set(peer_ids))!=len(ps) or any(not x.get("peer_id") or not x.get("origin") or not _digest(x.get("context_digest")) for x in ps): raise ResearchContractError("peer identity, uniqueness, origin, or digest is invalid")
    qp={x["peer_id"] for x in ps if x.get("federation_id")==request["federation_id"] and x.get("semantic_profile")==request["semantic_profile"] and int(x.get("checkpoint",0))==int(request["checkpoint"]) and int(x.get("source_count",0))>=int(request["minimum_source_quorum"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven","supported"}}; mp=set(peer_ids)-qp; unc={f"peer:{x}:not-qualified" for x in mp}
    selected:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); sources:set[str]=set(); om:set[str]=set(); neg:set[str]=set(); scores:list[int]=[]
    for x in rows:
        fid=x["fact_id"]; om.update(f"{fid}:{r}" for r in x.get("omission_reasons",[])); neg.add(f"{fid}:negative-result") if x.get("negative_result") else None; reasons=[]
        if x.get("semantic_profile")!=request["semantic_profile"]: reasons.append("semantic-profile-mismatch")
        missing=[t for t in request["required_claims"] if t not in x.get("terms",[])]; reasons.append("required-claim-missing") if missing else None; om.add(f"{fid}:missing-claims:{len(missing)}") if missing else None
        if x.get("replay_identity")!=request["replay_identity"]: reasons.append("replay-identity-mismatch")
        if x.get("signed") is not True or x.get("permitted") is not True: reasons.append("authorization-missing")
        if x.get("raw_data_local") is not True or x.get("aggregate_only") is not True: reasons.append("locality-or-aggregate-only-failed")
        if x.get("evidence_state")=="contradicted": blocked.add(fid); neg.add(f"{fid}:contradicted")
        elif x.get("evidence_state") not in {"proven","supported"} or reasons: unresolved.add(fid); unc.add(f"{fid}:unresolved")
        else: selected.add(fid); sources.add(x["source_id"]); scores.append(int(x.get("influence_milli",0))+int(x.get("freshness_milli",0)))
    global_block=not all(request.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only")); neg.add("request:policy-denied") if request.get("policy_allow") is not True else None; unc.add("request:protected-closure-incomplete") if request.get("protected_closure") is not True else None; unc.add("request:signed-approval-missing") if request.get("signed_approval") is not True else None; unc.add("request:federation-approval-missing") if request.get("federation_approved") is not True else None; unc.add("source:minimum-quorum-unmet") if len(sources)<int(request["minimum_source_quorum"]) else None
    disposition="blocked" if global_block or blocked else "unresolved" if not selected or len(sources)<int(request["minimum_source_quorum"]) or len(qp)<int(request["minimum_peer_quorum"]) else "qualified"
    if global_block: blocked.update(ids); selected.clear(); unresolved.clear(); scores.clear()
    om.add("request:context-gates-incomplete") if disposition!="qualified" else None
    so=sorted({x["source_id"] for x in rows}); sso=sorted(sources); mso=sorted(set(so)-set(sso)); payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"federation_id":request["federation_id"],"requester":request["requester"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"checkpoint":int(request["checkpoint"]),"disposition":disposition,"candidate_order":ids,"selected_order":sorted(selected),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"source_order":so,"selected_source_order":sso,"missing_source_order":mso,"peer_order":peer_ids,"qualified_peer_order":sorted(qp),"missing_peer_order":sorted(mp),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"influence_scores_milli":scores,"replay_identity":request["replay_identity"],"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload); result={**payload,"section_digest":digest,"artifact":{"artifact_id":f"certified-decision-section-1:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:permitted-context-summaries:{request['request_id']}",f"manage:local-capability:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True}; receipt=CertifiedDecisionSection1(result); receipt.validate(); return receipt

def idsContextCompilationDigest(receipt:CertifiedDecisionSection1)->str: receipt.validate(); return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","CertifiedDecisionSection1","context_compilation_manifest","operate_context_compilation","idsContextCompilationDigest"]
