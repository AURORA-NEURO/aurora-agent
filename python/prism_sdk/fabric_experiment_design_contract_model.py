"""Python parity for ``AFA-fabric-P09-F08`` experiment-design contract model."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-fabric-P09-F08"; CONTRACT_VERSION="fabric-federated-continual-experiment-design-contract-model/1.0"; INPUT_SCHEMA="ExperimentObjective4@1"; OUTPUT_SCHEMA="ExecutableExperimentDesign2@1"; CONTENT_TYPE="application/vnd.aurora.fabric-experiment-design-contract-2+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class ExecutableExperimentDesign2:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("compatibility") not in {"exact","additive","breaking"} or v.get("disposition") not in {"compatible","partial","blocked"} or not v.get("candidate_order") or not v.get("effect_receipts") or not all(str(v.get(k,"")).strip() for k in ("request_id","consumer","purpose","semantic_profile","required_schema")):raise ResearchContractError("design contract identity, compatibility, locality, or effects are incomplete")
        for k in ("candidate_order","compatible_order","unresolved_order","blocked_order","omitted_order","migration_order","semantic_loss_order","negative_evidence_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("design contract ordering is not canonical")
        ids=set(v["candidate_order"]);parts=[*v["compatible_order"],*v["unresolved_order"],*v["blocked_order"],*v["omitted_order"]]
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("design contract candidate states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","contract_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("contract_digest") or not all(_digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("design contract digest is invalid")
        if any(not e.startswith("observe:design-contract:") for e in v["effect_receipts"]):raise ResearchContractError("design contract effect is outside observation gate")
def experiment_design_contract_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"fabric","consumers":["agent developer","schema migration steward","experiment workflow compiler"],"behavior":"negotiate federated continual experiment-design schemas with deterministic compatibility and semantic-loss witnesses","value":"gives downstream agents a typed, replayable design contract without pretending a migrated envelope is an executable protocol","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":[],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def negotiate_experiment_design_contract(request:Mapping[str,Any])->ExecutableExperimentDesign2:
    if request.get("schema_version")!=INPUT_SCHEMA or not all(str(request.get(k,"")).strip() for k in ("request_id","consumer","purpose","semantic_profile","required_schema")) or not request.get("candidates") or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY:raise ResearchContractError("design contract request identity, candidates, replay, locality, or boundary is invalid")
    candidates=sorted({str(x.get("candidate_id","")) for x in request["candidates"]});
    if len(candidates)!=len(request["candidates"]) or any(not x.strip() for x in candidates):raise ResearchContractError("candidate ids must be unique and non-empty")
    q=set();u=set();b=set();o=set();m=set();sl=set();neg=set()
    for x in request["candidates"]:
        cid=str(x["candidate_id"]); xsrc=x.get("source_schema");xtgt=x.get("target_schema");exact=xsrc==request["required_schema"] and xtgt==request["required_schema"];additive=xtgt==request["required_schema"] and x.get("migration_available") is True
        if x.get("negative_result"):neg.add(cid)
        hard=x.get("permitted") is not True or x.get("signed") is not True or x.get("local_only") is not True or not _digest(x.get("artifact_digest")) or not _digest(x.get("provenance_digest")) or x.get("replay_identity")!=request["replay_identity"] or request.get("policy_allow") is not True or request.get("protected_closure") is not True
        if hard:b.add(cid)
        elif str(x.get("evidence_state")) in {"contradicted","unknown"}:u.add(cid);sl.add(f"{cid}:evidence-state")
        elif exact:q.add(cid)
        elif additive:q.add(cid);m.add(f"{cid}:additive-schema");sl.add(f"{cid}:bounded-migration")
        else:o.add(cid);m.add(f"{cid}:breaking-schema")
    compatibility="exact" if all(x.get("source_schema")==request["required_schema"] and x.get("target_schema")==request["required_schema"] for x in request["candidates"]) else "additive" if m and any("additive" in x for x in m) else "breaking";global_block=request.get("policy_allow") is not True or request.get("protected_closure") is not True;disp="blocked" if global_block or b else "partial" if u or o else "compatible";
    payload={"candidate_order":candidates,"compatible_order":sorted(q),"unresolved_order":sorted(u),"blocked_order":sorted(b),"omitted_order":sorted(o),"migration_order":sorted(m),"semantic_loss_order":sorted(sl),"negative_evidence_order":sorted(neg),"replay_identity":request["replay_identity"],"compatibility":compatibility,"disposition":disp};digest=_hash(payload);payload.update({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"required_schema":request["required_schema"],"contract_digest":digest,"artifact":{"artifact_id":f"fabric-experiment-design-contract:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":["contract-only; no executable dispatch"],"provenance_digests":sorted({str(x.get("provenance_digest")) for x in request["candidates"]}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"observe:design-contract:{request['request_id']}"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY});r=ExecutableExperimentDesign2(payload);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ExecutableExperimentDesign2","experiment_design_contract_manifest","negotiate_experiment_design_contract"]
