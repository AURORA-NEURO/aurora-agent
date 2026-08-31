"""Python parity for ``AFA-ids-P10-F19`` protocol simulation workbench."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P10-F19"; CONTRACT_VERSION="ids-prospective-high-throughput-protocol-simulation-research-workbench/1.0"; INPUT_SCHEMA="ProtocolWorkbenchRequest5@1"; OUTPUT_SCHEMA="ProtocolWorkbenchReport9@1"; CONTENT_TYPE="application/vnd.aurora.protocol-workbench-report-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return tuple(v)==tuple(sorted(set(v)))
@dataclass(frozen=True)
class ProtocolWorkbenchReport9:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","federation_id","protocol_id","requester","purpose","semantic_profile")) or int(v.get("checkpoint",0))<=0 or not v.get("stage_order") or not v.get("scenario_order") or not v.get("peer_order") or not v.get("batch_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("protocol workbench identity, locality, stages, scenarios, peers, batches, or effects are incomplete")
        fields=("stage_order","qualified_stage_order","unresolved_stage_order","blocked_stage_order","scenario_order","passed_scenario_order","failed_scenario_order","unknown_scenario_order","negative_scenario_order","peer_order","qualified_peer_order","missing_peer_order","batch_order","capacity_order","omission_order","uncertainty_order","negative_evidence_order","recovery_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("protocol workbench ordering is not canonical")
        stages=set(v["stage_order"]); parts=[*v["qualified_stage_order"],*v["unresolved_stage_order"],*v["blocked_stage_order"]]; scenarios=set(v["scenario_order"]); scenario_parts=[*v["passed_scenario_order"],*v["failed_scenario_order"],*v["unknown_scenario_order"]]; peers=set(v["peer_order"]); peer_parts=[*v["qualified_peer_order"],*v["missing_peer_order"]]
        if len(parts)!=len(stages) or set(parts)!=stages or len(set(parts))!=len(parts):raise ResearchContractError("stage states do not partition")
        if len(scenario_parts)!=len(scenarios) or set(scenario_parts)!=scenarios or len(set(scenario_parts))!=len(scenario_parts):raise ResearchContractError("scenario states do not partition")
        if len(peer_parts)!=len(peers) or set(peer_parts)!=peers or len(set(peer_parts))!=len(peer_parts):raise ResearchContractError("peer states do not partition")
        a=v.get("artifact",{}); ds=[v.get("replay_identity"),v.get("simulation_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]
        if not all(_digest(x) for x in ds) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("simulation_digest"):raise ResearchContractError("protocol workbench artifact or digest is invalid")
        if any(not e.startswith(("exchange:permitted-summaries:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("protocol workbench effect is outside the governed gate")
def protocol_workbench_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["high-throughput protocol scientist","preclinical workbench operator","federation steward"],"behavior":"simulates a bounded prospective protocol state machine across fault scenarios and aggregate peer summaries","value":"exposes capacity, recovery, evidence, and release gates before any laboratory effect","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:permitted-summaries","manage:local-capability"],"permissions":["read:local-protocol-manifests"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def simulate_protocol_workbench(req:Mapping[str,Any])->ProtocolWorkbenchReport9:
    if not all(str(req.get(k,"")).strip() for k in ("request_id","federation_id","protocol_id","requester","purpose","semantic_profile","required_protocol_version")) or not req.get("stages") or not req.get("scenarios") or not req.get("peers") or int(req.get("checkpoint",0))<=0 or int(req.get("batch_size",0))<=0 or int(req.get("max_budget_units",0))<=0 or int(req.get("minimum_peer_quorum",0))<=0 or req.get("boundary")!=PRECLINICAL_BOUNDARY or req.get("raw_data_local") is not True or req.get("aggregate_only") is not True or not _digest(req.get("replay_identity")):raise ResearchContractError("protocol workbench request identity, bounds, stages, scenarios, peers, budget, locality, replay, or boundary is invalid")
    stages=sorted((dict(x) for x in req["stages"]),key=lambda x:(int(x.get("sequence",0)),str(x.get("stage_id","")))); ids=[str(x.get("stage_id","")) for x in stages]; scenarios=sorted((dict(x) for x in req["scenarios"]),key=lambda x:str(x.get("scenario_id",""))); sids=[str(x.get("scenario_id","")) for x in scenarios]; peers=sorted((dict(x) for x in req["peers"]),key=lambda x:str(x.get("peer_id",""))); pids=[str(x.get("peer_id","")) for x in peers]
    if len(set(ids))!=len(ids) or len(set(sids))!=len(sids) or len(set(pids))!=len(pids) or any(not x.get("stage_id") or not x.get("input_schema") or not x.get("output_schema") or not x.get("effect_class") or int(x.get("estimated_units",0))<=0 or not all(_digest(x.get(k)) for k in ("artifact_digest","provenance_digest")) for x in stages) or any(not x.get("scenario_id") or not x.get("fault_class") or not _digest(x.get("replay_digest")) for x in scenarios) or any(not x.get("peer_id") or not x.get("origin") or not _digest(x.get("report_digest")) for x in peers):raise ResearchContractError("protocol workbench stage, scenario, or peer identity is invalid")
    qs:set[str]=set(); us:set[str]=set(); bs:set[str]=set(); unc:set[str]=set(); neg_e:set[str]=set(); total=0
    for x in stages:
        sid=x["stage_id"]; total+=int(x["estimated_units"]); reasons=[]; state=x.get("evidence_state")
        if state=="contradicted":reasons.append("contradicted");neg_e.add(f"stage:{sid}:contradicted")
        if state not in {"proven","supported"}:reasons.append("evidence-unresolved");unc.add(f"stage:{sid}:evidence-state")
        if x.get("deterministic") is not True:reasons.append("nondeterministic")
        if x.get("local_only") is not True:reasons.append("not-local")
        if "contradicted" in reasons or "not-local" in reasons:bs.add(sid)
        elif not reasons:qs.add(sid)
        else:us.add(sid)
    passed:set[str]=set(); failed:set[str]=set(); unknown:set[str]=set(); negative:set[str]=set(); omissions:set[str]=set(); recovery:set[str]=set(); capacity:set[str]=set()
    for x in scenarios:
        sid=x["scenario_id"]; state=x.get("observed_state");
        if x.get("negative_result"):negative.add(sid);neg_e.add(f"scenario:{sid}:negative-result")
        if int(x.get("budget_units",0))>int(req["max_budget_units"]):failed.add(sid);capacity.add(f"scenario:{sid}:budget-exceeded");continue
        if not str(x.get("expected_recovery","")).strip():omissions.add(f"scenario:{sid}:missing-recovery-plan")
        if state in {"proven","supported"}:
            if all(a in qs for a in x.get("affected_stages",[])):passed.add(sid)
            else:failed.add(sid);recovery.add(f"{sid}:blocked-stage-recovery")
        elif state=="contradicted":failed.add(sid);neg_e.add(f"scenario:{sid}:contradicted")
        else:unknown.add(sid);unc.add(f"scenario:{sid}:evidence-state")
    qp={x["peer_id"] for x in peers if x.get("protocol_id")==req["protocol_id"] and x.get("semantic_profile")==req["semantic_profile"] and int(x.get("checkpoint",0))==int(req["checkpoint"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven","supported"}}; mp=set(pids)-qp; unc.update(f"peer:{p}:not-qualified" for p in mp); batches=[f"batch:{i:04d}" for i in range((len(ids)+int(req["batch_size"])-1)//int(req["batch_size"]))]
    if total>int(req["max_budget_units"]):capacity.add(f"request:total-budget-exceeded:{total}")
    if len(qp)<int(req["minimum_peer_quorum"]):unc.add("peer:minimum-quorum-unmet")
    global_block=not all(req.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only")); neg_e.add("request:policy-denied") if req.get("policy_allow") is not True else None; unc.add("request:protected-closure-incomplete") if req.get("protected_closure") is not True else None; unc.add("request:signed-approval-missing") if req.get("signed_approval") is not True else None; unc.add("request:federation-approval-missing") if req.get("federation_approved") is not True else None
    disposition="blocked" if global_block or bs else "unresolved" if len(qp)<int(req["minimum_peer_quorum"]) or failed or unknown or not qs or capacity else "qualified"
    if disposition!="qualified":omissions.add("request:simulation-not-release-ready")
    if global_block:bs.update(ids);qs.clear();us.clear();passed.clear();failed.clear();unknown.clear()
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":req["request_id"],"federation_id":req["federation_id"],"protocol_id":req["protocol_id"],"requester":req["requester"],"purpose":req["purpose"],"semantic_profile":req["semantic_profile"],"checkpoint":int(req["checkpoint"]),"disposition":disposition,"stage_order":ids,"qualified_stage_order":sorted(qs),"unresolved_stage_order":sorted(us),"blocked_stage_order":sorted(bs),"scenario_order":sids,"passed_scenario_order":sorted(passed),"failed_scenario_order":sorted(failed),"unknown_scenario_order":sorted(unknown),"negative_scenario_order":sorted(negative),"peer_order":pids,"qualified_peer_order":sorted(qp),"missing_peer_order":sorted(mp),"batch_order":batches,"capacity_order":sorted(capacity),"omission_order":sorted(omissions),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg_e),"recovery_order":sorted(recovery),"total_units":total,"replay_identity":req["replay_identity"],"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload); result={**payload,"simulation_digest":digest,"artifact":{"artifact_id":f"protocol-workbench-report-9:{req['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in stages}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:permitted-summaries:{req['request_id']}",f"manage:local-capability:{req['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True}; receipt=ProtocolWorkbenchReport9(result);receipt.validate();return receipt
def idsProtocolSimulationWorkbenchDigest(receipt:ProtocolWorkbenchReport9)->str:receipt.validate();return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ProtocolWorkbenchReport9","protocol_workbench_manifest","simulate_protocol_workbench","idsProtocolSimulationWorkbenchDigest"]
