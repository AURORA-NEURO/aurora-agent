"""Python parity surface for AFA-adapter-P02-F29."""
from __future__ import annotations
from dataclasses import dataclass
import re
from typing import Any, Mapping
from .adapter_local_retrieval_synthesis_research_workbench import render_local_retrieval_synthesis_research_workbench
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEATURE_ID="AFA-adapter-P02-F29"; CONTRACT_VERSION="adapter-local-retrieval-synthesis-federated-control-plane/1.0"; INPUT_SCHEMA="ScopedRetrievalQuery1@1"; OUTPUT_SCHEMA="EvidenceSynthesis5@1"
@dataclass(frozen=True)
class LocalRetrievalSynthesisFederatedControlPlaneReceipt:
    request_id:str; service_id:str; node_id:str; capacity:int; active_runs:int; admission:str; scope:str; workbench_digest:str; health_digest:str; replay_identity:str; control_digest:str; omissions:tuple[str,...]; uncertainty:tuple[str,...]; counterexamples:tuple[str,...]; effect_receipts:tuple[str,...]; artifact:dict[str,Any]; schema_version:str=RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version:str=CONTRACT_VERSION; feature_id:str=FEATURE_ID; raw_data_local:bool=True; boundary:str=PRECLINICAL_BOUNDARY
    def validate(self)->None:
        if (self.schema_version,self.contract_version,self.feature_id)!=(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID) or self.boundary!=PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.service_id.strip() or not self.node_id.strip() or not self.scope.strip() or self.capacity<=0 or self.active_runs>self.capacity or self.admission not in {"admitted","degraded","approval_required","blocked"} or not self.effect_receipts: raise ResearchContractError("control-plane identity, capacity, admission, locality, or effects are incomplete")
        for values in (self.omissions,self.uncertainty,self.counterexamples,self.effect_receipts):
            if tuple(sorted(set(values)))!=values: raise ResearchContractError("control-plane ordering is not canonical")
        for value in (self.workbench_digest,self.health_digest,self.replay_identity,self.control_digest,self.artifact.get("content_hash")):
            if not isinstance(value,str) or not re.fullmatch(r"[0-9a-f]{64}",value): raise ResearchContractError("control-plane digest is invalid")
        if any(not e.startswith("operate:local-federated-control-plane:") and not e.startswith("approval-required:") and e!="block:unsafe-release" for e in self.effect_receipts): raise ResearchContractError("control-plane effect is outside admission gate")
def operate_local_retrieval_synthesis_federated_control_plane(*,workbench_kwargs:Mapping[str,Any],service_id:str,node_id:str,capacity:int,active_runs:int,policy_allow:bool,protected_closure:bool,signed_approval:bool,federation_permitted:bool,health_digest:str,replay_identity:str,boundary:str=PRECLINICAL_BOUNDARY)->LocalRetrievalSynthesisFederatedControlPlaneReceipt:
    if not service_id.strip() or not node_id.strip() or capacity<=0 or active_runs>capacity or not re.fullmatch(r"[0-9a-f]{64}",health_digest) or not re.fullmatch(r"[0-9a-f]{64}",replay_identity) or boundary!=PRECLINICAL_BOUNDARY: raise ResearchContractError("control-plane identity, capacity, replay, health, or boundary is invalid")
    wb=render_local_retrieval_synthesis_research_workbench(**dict(workbench_kwargs))
    if wb.replay_identity!=replay_identity: raise ResearchContractError("control-plane replay identity does not match workbench")
    omissions=set(wb.omissions); uncertainty=set(wb.uncertainty); counter=set()
    if not policy_allow: counter.add("policy authorization denied"); omissions.add("policy authorization")
    if not protected_closure: counter.add("protected closure incomplete"); uncertainty.add("protected closure")
    if not signed_approval: counter.add("signed approval missing"); uncertainty.add("signed approval")
    if not federation_permitted: counter.add("federation permission denied"); omissions.add("federation permission")
    saturated=active_runs*100>=capacity*90
    if saturated: uncertainty.add("capacity headroom is exhausted")
    admission="blocked" if not policy_allow or not federation_permitted else ("approval_required" if not protected_closure or not signed_approval else ("degraded" if saturated else "admitted")); effect=f"operate:local-federated-control-plane:{service_id}" if admission=="admitted" else (f"approval-required:{service_id}" if admission=="approval_required" else "block:unsafe-release")
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":wb.request_id,"service_id":service_id,"node_id":node_id,"capacity":capacity,"active_runs":active_runs,"admission":admission,"scope":wb.scope,"workbench_digest":wb.workbench_digest,"health_digest":health_digest,"replay_identity":replay_identity,"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"counterexamples":sorted(counter),"raw_data_local":True,"boundary":PRECLINICAL_BOUNDARY}; control_digest=research_artifact_digest(payload); receipt=LocalRetrievalSynthesisFederatedControlPlaneReceipt(request_id=wb.request_id,service_id=service_id,node_id=node_id,capacity=capacity,active_runs=active_runs,admission=admission,scope=wb.scope,workbench_digest=wb.workbench_digest,health_digest=health_digest,replay_identity=replay_identity,control_digest=control_digest,omissions=tuple(sorted(omissions)),uncertainty=tuple(sorted(uncertainty)),counterexamples=tuple(sorted(counter)),effect_receipts=(effect,),artifact={"content_hash":research_artifact_digest({**payload,"control_digest":control_digest}),"media_type":"application/vnd.aurora.local-retrieval-synthesis-federated-control-plane+json"}); receipt.validate(); return receipt
__all__=["FEATURE_ID","CONTRACT_VERSION","LocalRetrievalSynthesisFederatedControlPlaneReceipt","operate_local_retrieval_synthesis_federated_control_plane"]
