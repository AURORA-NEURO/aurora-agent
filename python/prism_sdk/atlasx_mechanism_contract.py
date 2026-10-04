"""Python parity for ``AFA-atlasx-P08-F08`` mechanism contract model."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-atlasx-P08-F08"; CONTRACT_VERSION="atlasx-federated-continual-mechanism-contract-model/1.0"; INPUT_SCHEMA="MechanismQuestion4@1"; OUTPUT_SCHEMA="MechanismPortfolio2@1"; CONTENT_TYPE="application/vnd.aurora.atlasx-mechanism-portfolio-2+json"; MAX_CANDIDATES=4096; MAX_PEERS=512
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class AtlasxMechanismPortfolio2:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","question_id","purpose","semantic_profile")) or not v.get("required_mechanism_order") or not v.get("mechanism_order") or not v.get("peer_order") or not v.get("effect_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("mechanism identity, requirements, candidates, peers, effects, locality, or disposition is incomplete")
        keys=("required_mechanism_order","mechanism_order","selected_mechanism_order","unresolved_mechanism_order","blocked_mechanism_order","peer_order","qualified_peer_order","missing_peer_order","evidence_order","omission_order","uncertainty_order","contradiction_order","negative_evidence_order","effect_order","effect_receipts")
        for k in keys:
            if not _ordered(v.get(k,[])):raise ResearchContractError("mechanism ordering is not canonical")
        mids=set(v["mechanism_order"]);parts=v["selected_mechanism_order"]+v["unresolved_mechanism_order"]+v["blocked_mechanism_order"];pids=set(v["peer_order"]);pparts=v["qualified_peer_order"]+v["missing_peer_order"]
        if len(mids)!=len(v["mechanism_order"]) or len(parts)!=len(mids) or set(parts)!=mids or len(pids)!=len(v["peer_order"]) or len(pparts)!=len(pids) or set(pparts)!=pids or len(v.get("support_milli_order",[]))!=len(v["mechanism_order"]) or len(v.get("novelty_milli_order",[]))!=len(v["mechanism_order"]):raise ResearchContractError("mechanism or peer states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","portfolio_digest")) or a.get("content_hash")!=v.get("portfolio_digest") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(d) for d in a.get("provenance_digests",[])):raise ResearchContractError("mechanism digest or artifact metadata is invalid")
        if any(not e.startswith(("view:atlasx-mechanism-portfolio:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("effect is outside the atlasx mechanism contract gate")
    def digest(self)->str:self.validate();return _hash(self.to_dict())
def mechanism_contract_model_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"atlasx","consumers":["imaging core scientist","mechanism scientist","federation steward"],"behavior":"normalize mechanism candidates and signed peer attestations into a typed, digest-addressed portfolio with deterministic compatibility and evidence states","value":"keeps competing explanations, omissions, and negative evidence visible while enabling safe cross-institution exchange of minimal summaries","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","question_id","purpose","semantic_profile")) or not r.get("required_mechanism_order") or not r.get("candidates") or not r.get("peers") or len(r["candidates"])>MAX_CANDIDATES or len(r["peers"])>MAX_PEERS or not isinstance(r.get("minimum_peer_quorum"),int) or r["minimum_peer_quorum"]<1 or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("mechanism identity, closure, bounds, quorum, replay, locality, or boundary is invalid")
    if len(set(r["required_mechanism_order"]))!=len(r["required_mechanism_order"]) or any(not isinstance(x,str) or not x.strip() for x in r["required_mechanism_order"]):raise ResearchContractError("required mechanisms must be unique and non-empty")
    mids=set()
    for c in r["candidates"]:
        if not isinstance(c,Mapping) or not isinstance(c.get("mechanism_id"),str) or not c["mechanism_id"].strip() or c["mechanism_id"] in mids or not isinstance(c.get("study_id"),str) or not c["study_id"].strip() or not isinstance(c.get("modality"),str) or not c["modality"].strip() or not isinstance(c.get("statement"),str) or not c["statement"].strip() or not isinstance(c.get("support_milli"),int) or c["support_milli"]<0 or c["support_milli"]>1000 or not isinstance(c.get("novelty_milli"),int) or c["novelty_milli"]<0 or c["novelty_milli"]>1000 or c.get("evidence_state") not in {"proven","supported","unknown","unmeasured","contradicted"} or not _digest(c.get("artifact_digest")) or not _digest(c.get("provenance_digest")) or not _digest(c.get("replay_identity")) or c.get("local") is not True or c.get("aggregate_only") is not True:raise ResearchContractError(f"candidate {c.get('mechanism_id','')} is invalid, duplicated, non-local, or not digest-bound")
        mids.add(c["mechanism_id"])
    pids=set()
    for p in r["peers"]:
        if not isinstance(p,Mapping) or not isinstance(p.get("peer_id"),str) or not p["peer_id"].strip() or p["peer_id"] in pids or not isinstance(p.get("mechanism_id"),str) or not p["mechanism_id"].strip() or not isinstance(p.get("semantic_profile"),str) or not p["semantic_profile"].strip() or p.get("evidence_state") not in {"proven","supported","unknown","unmeasured","contradicted"} or not isinstance(p.get("authorized"),bool) or not _digest(p.get("artifact_digest")) or not _digest(p.get("provenance_digest")) or not _digest(p.get("replay_identity")) or p.get("local") is not True or p.get("aggregate_only") is not True:raise ResearchContractError(f"peer {p.get('peer_id','')} is invalid, duplicated, non-local, or not digest-bound")
        pids.add(p["peer_id"])
def admit_atlasx_mechanism_contract(r:Mapping[str,Any])->AtlasxMechanismPortfolio2:
    _validate_request(r);cands=sorted((dict(c) for c in r["candidates"]),key=lambda c:c["mechanism_id"]);nodes=[c["mechanism_id"] for c in cands];peers=sorted((dict(p) for p in r["peers"]),key=lambda p:p["peer_id"]);peer_order=[p["peer_id"] for p in peers];qualified=[p["peer_id"] for p in peers if p["semantic_profile"]==r["semantic_profile"] and p["replay_identity"]==r["replay_identity"] and p["authorized"] is True and p["evidence_state"] in {"proven","supported"}];missing=sorted(set(peer_order)-set(qualified));quorum_ok=len(qualified)>=r["minimum_peer_quorum"];selected=set();unresolved=set();blocked=set();evidence=set();omissions=set();uncertainty=set();contradiction=set();negative=set()
    for c in cands:
        i=c["mechanism_id"]
        if c["evidence_state"]=="contradicted":blocked.add(i);contradiction.add(i);negative.add(f"{i}:contradicted")
        elif c["evidence_state"] in {"unknown","unmeasured"}:unresolved.add(i);evidence.add(i);uncertainty.add(f"{i}:evidence-state")
        else:selected.add(i)
    if not quorum_ok:
        for i in list(selected):selected.remove(i);unresolved.add(i);uncertainty.add(f"{i}:peer-quorum")
        omissions.add("request:peer-quorum-not-met")
    node_set=set(nodes);required_missing=any(i not in node_set for i in r["required_mechanism_order"])
    for i in r["required_mechanism_order"]:
        if i not in node_set:omissions.add(f"mechanism:{i}:missing");negative.add(f"mechanism:{i}:no-candidate")
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
    if global_block:blocked.update(nodes);selected.clear();unresolved.clear();omissions.add("request:governance-or-locality-denied")
    so=sorted(selected);uo=sorted(unresolved);bo=sorted(blocked);disp="blocked" if global_block or bo else "unresolved" if required_missing or uo or missing else "qualified"
    if disp!="qualified":omissions.add("request:mechanism-contract-not-closed")
    effects=["manage:local-capability","view:atlasx-mechanism-portfolio"] if disp=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"question_id":r["question_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"disposition":disp,"required_mechanism_order":r["required_mechanism_order"],"mechanism_order":nodes,"selected_mechanism_order":so,"unresolved_mechanism_order":uo,"blocked_mechanism_order":bo,"peer_order":peer_order,"qualified_peer_order":sorted(qualified),"missing_peer_order":missing,"support_milli_order":[c["support_milli"] for c in cands],"novelty_milli_order":[c["novelty_milli"] for c in cands],"evidence_order":sorted(evidence),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"contradiction_order":sorted(contradiction),"negative_evidence_order":sorted(negative),"effect_order":effects,"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);v={**payload,"portfolio_digest":digest,"artifact":{"artifact_id":f"atlasx-mechanism-portfolio-2:{r['question_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":sorted({c["provenance_digest"] for c in cands}|{p["provenance_digest"] for p in peers}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":sorted(e if e=="block:unsafe-release" else f"{e}:{r['request_id']}" for e in effects)};receipt=AtlasxMechanismPortfolio2(v);receipt.validate();return receipt
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","AtlasxMechanismPortfolio2","mechanism_contract_model_manifest","admit_atlasx_mechanism_contract"]
