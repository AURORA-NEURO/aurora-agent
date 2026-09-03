"""Python parity for ``AFA-packs-P10-F18`` protocol simulation workbench."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .ids_protocol_simulation_workbench import simulate_protocol_workbench as _ids_simulate
FEATURE_ID="AFA-packs-P10-F18"; CONTRACT_VERSION="packs-multimodal-multi-study-protocol-simulation-research-workbench/1.0"; INPUT_SCHEMA="ProtocolWorkbenchRequest5@1"; OUTPUT_SCHEMA="ProtocolWorkbenchReport9@1"; CONTENT_TYPE="application/vnd.aurora.packs-protocol-workbench-report-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class PacksProtocolWorkbenchReport9:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","federation_id","protocol_id","requester","purpose","semantic_profile")) or int(v.get("checkpoint",0))<=0 or not v.get("stage_order") or not v.get("scenario_order") or not v.get("peer_order") or not v.get("batch_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("packs protocol workbench identity, locality, stages, scenarios, peers, batches, or effects are incomplete")
        fields=("stage_order","qualified_stage_order","unresolved_stage_order","blocked_stage_order","scenario_order","passed_scenario_order","failed_scenario_order","unknown_scenario_order","negative_scenario_order","peer_order","qualified_peer_order","missing_peer_order","batch_order","capacity_order","omission_order","uncertainty_order","negative_evidence_order","recovery_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("packs protocol workbench ordering is not canonical")
        stages=set(v["stage_order"]);parts=[*v["qualified_stage_order"],*v["unresolved_stage_order"],*v["blocked_stage_order"]];scenarios=set(v["scenario_order"]);sp=[*v["passed_scenario_order"],*v["failed_scenario_order"],*v["unknown_scenario_order"]];peers=set(v["peer_order"]);pp=[*v["qualified_peer_order"],*v["missing_peer_order"]]
        if len(parts)!=len(stages) or set(parts)!=stages or len(set(parts))!=len(parts) or len(sp)!=len(scenarios) or set(sp)!=scenarios or len(set(sp))!=len(sp) or len(pp)!=len(peers) or set(pp)!=peers or len(set(pp))!=len(pp):raise ResearchContractError("packs protocol states do not partition")
        if not all(_digest(x) for x in (v.get("replay_identity"),v.get("simulation_digest"),a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("simulation_digest"):raise ResearchContractError("packs protocol artifact or digest is invalid")
        if any(e!="block:unsafe-release" and not e.startswith("view:packs-protocol-workbench:") for e in v["effect_receipts"]):raise ResearchContractError("packs protocol effect is outside workbench gate")
def packs_protocol_workbench_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"packs","consumers":["protocol scientist","preclinical workbench operator","benchmark curator"],"behavior":"simulate multimodal multi-study protocol state machines and fault scenarios through a deterministic researcher workbench","value":"exposes protocol capacity, recovery, evidence, peer, and release gates before laboratory integration","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["view:packs-protocol-workbench"],"permissions":["read:local-protocol-manifests"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def simulate_packs_protocol_workbench(request:Mapping[str,Any])->PacksProtocolWorkbenchReport9:
    value=_ids_simulate(request).to_dict();value["contract_version"]=CONTRACT_VERSION;value["feature_id"]=FEATURE_ID;value["artifact"]["content_type"]=CONTENT_TYPE;value["effect_receipts"]=[f"view:packs-protocol-workbench:{request['protocol_id']}"] if value.get("disposition")=="qualified" else ["block:unsafe-release"];base={k:v for k,v in value.items() if k not in {"simulation_digest","artifact","effect_receipts"}};d=_hash(base);value["simulation_digest"]=d;value["artifact"]["content_hash"]=d;r=PacksProtocolWorkbenchReport9(value);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","PacksProtocolWorkbenchReport9","packs_protocol_workbench_manifest","simulate_packs_protocol_workbench"]
