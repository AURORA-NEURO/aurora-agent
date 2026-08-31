"""Python parity for ``AFA-worldfactory-P10-F32``.

This surface simulates caller-declared protocol state machines and fault scenarios;
it never dispatches instruments or exports raw observations.
"""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-worldfactory-P10-F32"
CONTRACT_VERSION = "worldfactory-federated-continual-protocol-simulation-federated-control-plane/1.0"
INPUT_SCHEMA = "ProtocolDraft4@1"
OUTPUT_SCHEMA = "ProtocolSimulationReport8@1"
CONTENT_TYPE = "application/vnd.aurora.protocol-simulation-report-8+json"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: list[str]) -> bool: return tuple(values) == tuple(sorted(set(values)))

@dataclass(frozen=True)
class ProtocolSimulationReport8:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        v=self.value
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k," ")).strip() for k in ("request_id","federation_id","protocol_id","requester","purpose","semantic_profile")) or not v.get("stage_order") or not v.get("scenario_order") or not v.get("peer_order") or not v.get("effect_receipts") or v.get("checkpoint",0)<=0 or v.get("disposition") not in {"qualified","unresolved","blocked"}: raise ResearchContractError("protocol simulation identity, checkpoint, locality, stages, scenarios, peers, or effects are incomplete")
        fields=("stage_order","qualified_stage_order","unresolved_stage_order","blocked_stage_order","scenario_order","passed_scenario_order","failed_scenario_order","unknown_scenario_order","negative_scenario_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","recovery_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields): raise ResearchContractError("protocol simulation ordering is not canonical")
        if set(v["stage_order"]) != set(v["qualified_stage_order"])|set(v["unresolved_stage_order"])|set(v["blocked_stage_order"]): raise ResearchContractError("protocol stage dispositions do not partition")
        if set(v["scenario_order"]) != set(v["passed_scenario_order"])|set(v["failed_scenario_order"])|set(v["unknown_scenario_order"]): raise ResearchContractError("protocol scenario dispositions do not partition")
        if set(v["peer_order"]) != set(v["qualified_peer_order"])|set(v["missing_peer_order"]): raise ResearchContractError("protocol peer dispositions do not partition")
        a=v.get("artifact",{}); digests=[v.get("replay_identity"),v.get("simulation_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]
        if not all(_digest(x) for x in digests) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("simulation_digest"): raise ResearchContractError("protocol simulation artifact or digest is invalid")
        if any(not e.startswith(("manage:local-capability:","exchange:permitted-summaries:")) and e!="block:unsafe-release" for e in v["effect_receipts"]): raise ResearchContractError("protocol simulation effect is outside the governed gate")

def protocol_simulation_manifest() -> dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"worldfactory","consumers":["preclinical neuroscientist","protocol operator","federation steward"],"behavior":"simulates a declared protocol state machine across bounded fault scenarios and peer summaries","value":"makes protocol robustness, recovery, and federation gates auditable before any laboratory effect","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability","exchange:permitted-summaries"],"permissions":["operate:institution-node"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}

def simulate_protocol(draft: Mapping[str,Any]) -> ProtocolSimulationReport8:
    req=draft
    if not all(str(req.get(k,"")).strip() for k in ("request_id","federation_id","protocol_id","requester","purpose","semantic_profile","required_protocol_version")) or int(req.get("checkpoint",0))<=0 or not req.get("stages") or not req.get("scenarios") or not req.get("peers") or int(req.get("max_budget_units",0))<=0 or int(req.get("minimum_peer_quorum",0))<=0 or req.get("boundary")!=PRECLINICAL_BOUNDARY or req.get("raw_data_local") is not True or req.get("aggregate_only") is not True or not _digest(req.get("replay_identity")): raise ResearchContractError("protocol simulation request identity, bounds, stages, scenarios, peers, budget, locality, replay, or boundary is invalid")
    stages=sorted((dict(x) for x in req["stages"]),key=lambda x:(int(x.get("sequence",0)),str(x.get("stage_id",""))))
    stage_ids=[str(x.get("stage_id","")) for x in stages]
    if len(set(stage_ids))!=len(stage_ids) or any(not x.get("stage_id") or not _digest(x.get("artifact_digest")) or not _digest(x.get("provenance_digest")) or int(x.get("estimated_units",0))<=0 for x in stages): raise ResearchContractError("protocol stage identity or digest is invalid")
    peers=sorted((dict(x) for x in req["peers"]),key=lambda x:str(x.get("peer_id",""))); peer_ids=[str(x.get("peer_id","")) for x in peers]
    if len(set(peer_ids))!=len(peer_ids) or any(not x.get("peer_id") or not x.get("origin") or not _digest(x.get("report_digest")) for x in peers): raise ResearchContractError("protocol peer identity or digest is invalid")
    qualified_peers={x["peer_id"] for x in peers if x.get("protocol_id")==req["protocol_id"] and x.get("semantic_profile")==req["semantic_profile"] and int(x.get("checkpoint",0))==int(req["checkpoint"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven","supported"}}
    missing_peers=set(peer_ids)-qualified_peers; uncertainty={f"peer:{x}:not-qualified" for x in missing_peers}; uncertainty.update(f"peer:{x['peer_id']}:contradicted" for x in peers if x.get("evidence_state")=="contradicted")
    qs=set(); us=set(); bs=set(); neg_e=set(); total=0
    for x in stages:
        total+=int(x.get("estimated_units",0)); state=x.get("evidence_state"); reasons=[]
        if state=="contradicted": reasons.append("contradicted-evidence"); neg_e.add(f"stage:{x['stage_id']}:contradicted")
        if state not in {"proven","supported"}: reasons.append("evidence-state-unresolved"); uncertainty.add(f"stage:{x['stage_id']}:evidence-state")
        if x.get("deterministic") is not True: reasons.append("nondeterministic-stage")
        if x.get("local_only") is not True: reasons.append("stage-not-local")
        (bs if any(r in {"contradicted-evidence","stage-not-local"} for r in reasons) else us if reasons else qs).add(x["stage_id"])
    scenarios=sorted((dict(x) for x in req["scenarios"]),key=lambda x:str(x.get("scenario_id",""))); scenario_ids=[str(x.get("scenario_id","")) for x in scenarios]; passed=set(); failed=set(); unknown=set(); negative=set(); omissions=set(); recovery=set()
    for x in scenarios:
        sid=x["scenario_id"]
        if x.get("negative_result"): negative.add(sid); neg_e.add(f"scenario:{sid}:negative-result")
        if int(x.get("budget_units",0))>int(req["max_budget_units"]): failed.add(sid); omissions.add(f"scenario:{sid}:budget-exceeded"); continue
        state=x.get("observed_state")
        if state in {"proven","supported"} and all(y in qs for y in x.get("affected_stages",[])): passed.add(sid)
        elif state in {"proven","supported"}: failed.add(sid); recovery.add(f"{sid}:blocked-stage-recovery")
        elif state=="contradicted": failed.add(sid); neg_e.add(f"scenario:{sid}:contradicted")
        else: unknown.add(sid); uncertainty.add(f"scenario:{sid}:evidence-state")
        if not str(x.get("expected_recovery","")).strip(): omissions.add(f"scenario:{sid}:missing-recovery-plan")
    if len(qualified_peers)<int(req["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    global_block=not all(req.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
    if req.get("policy_allow") is not True: neg_e.add("request:policy-denied")
    if req.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if req.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if req.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    disposition="blocked" if global_block or bs else "unresolved" if len(qualified_peers)<int(req["minimum_peer_quorum"]) or failed or unknown or not qs else "qualified"; omissions.add("request:simulation-not-release-ready") if disposition!="qualified" else None
    so=stage_ids; qo=sorted(qs); uo=sorted(us); bo=sorted(bs); co=scenario_ids; po=sorted(passed); fo=sorted(failed); uco=sorted(unknown); no=sorted(negative); pqo=sorted(qualified_peers); pmo=sorted(missing_peers); oo=sorted(omissions); uo2=sorted(uncertainty); neo=sorted(neg_e); ro=sorted(recovery)
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":req["request_id"],"federation_id":req["federation_id"],"protocol_id":req["protocol_id"],"requester":req["requester"],"purpose":req["purpose"],"semantic_profile":req["semantic_profile"],"checkpoint":int(req["checkpoint"]),"disposition":disposition,"stage_order":so,"qualified_stage_order":qo,"unresolved_stage_order":uo,"blocked_stage_order":bo,"scenario_order":co,"passed_scenario_order":po,"failed_scenario_order":fo,"unknown_scenario_order":uco,"negative_scenario_order":no,"peer_order":peer_ids,"qualified_peer_order":pqo,"missing_peer_order":pmo,"omission_order":oo,"uncertainty_order":uo2,"negative_evidence_order":neo,"recovery_order":ro,"total_units":total,"replay_identity":req["replay_identity"],"boundary":PRECLINICAL_BOUNDARY}
    digest=_hash(payload); result={**payload,"simulation_digest":digest,"artifact":{"artifact_id":f"protocol-simulation-report-8:{req['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in stages}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"exchange:permitted-summaries:{req['request_id']}",f"manage:local-capability:{req['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True}
    receipt=ProtocolSimulationReport8(result); receipt.validate(); return receipt

def worldfactoryProtocolSimulationDigest(receipt: ProtocolSimulationReport8) -> str: receipt.validate(); return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ProtocolSimulationReport8","protocol_simulation_manifest","simulate_protocol","worldfactoryProtocolSimulationDigest"]
