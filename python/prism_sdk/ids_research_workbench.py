"""Python parity for ``AFA-ids-P24-F18`` researcher/admin workbench."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P24-F18"; CONTRACT_VERSION="ids-multimodal-researcher-admin-research-workbench/1.0"; INPUT_SCHEMA="ResearchWorkspaceState7@1"; OUTPUT_SCHEMA="InteractiveResearchWorkspace9@1"; CONTENT_TYPE="application/vnd.aurora.interactive-research-workspace-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class InteractiveResearchWorkspace9:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(v.get(k," ").strip() for k in ("request_id","workspace_id","purpose","semantic_profile","comparability_key")) or not v.get("panel_order") or not v.get("view_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("workspace identity, locality, panels, views, disposition, or effects are incomplete")
        fields=("panel_order","selected_panel_order","unresolved_panel_order","blocked_panel_order","missing_study_order","missing_modality_order","omission_order","uncertainty_order","negative_evidence_order","view_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("workspace ordering is not canonical")
        ids=set(v["panel_order"]);parts=v["selected_panel_order"]+v["unresolved_panel_order"]+v["blocked_panel_order"]
        if len(ids)!=len(v["panel_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("panel states do not partition")
        if not all(_digest(d) for d in [v.get("replay_identity"),v.get("workspace_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]) or a.get("content_hash")!=v.get("workspace_digest") or a.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("workspace digest or artifact metadata is inconsistent")
        if any(not e.startswith(("view:research-workspace:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("workspace effect is outside governed gate")
def research_workbench_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["preclinical researcher","study administrator","comparability reviewer","provenance auditor"],"behavior":"compile a multimodal researcher/admin workspace view with explicit omissions, uncertainty, provenance, and locality","value":"gives researchers an auditable cross-study workspace without hiding missing modalities, contradictory evidence, or protected data","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["view:research-workspace","manage:local-capability"],"permissions":["read:local-research-summaries","request:workspace-view"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","workspace_id","purpose","semantic_profile","comparability_key")) or not r.get("required_studies") or not r.get("required_modalities") or not r.get("panels") or len(r["panels"])>16384 or int(r.get("selected_panel_limit",0))<=0 or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("workspace identity, required closure, panel bound, replay, or locality is invalid")
    if len(set(r["required_studies"]))!=len(r["required_studies"]) or len(set(r["required_modalities"]))!=len(r["required_modalities"]):raise ResearchContractError("required studies or modalities are not unique")
    ids:set[str]=set()
    for p in r["panels"]:
        if not all(isinstance(p.get(k),str) and p[k].strip() for k in ("panel_id","study_id","modality","comparability_key")) or not _digest(p.get("content_digest")) or not _digest(p.get("provenance_digest")) or not _digest(p.get("replay_identity")) or p["panel_id"] in ids:raise ResearchContractError("panel identity, comparability, digest, or uniqueness is invalid")
        ids.add(p["panel_id"])
def compile_research_workbench(r:Mapping[str,Any])->InteractiveResearchWorkspace9:
    _validate_request(r);panels=sorted((dict(p) for p in r["panels"]),key=lambda p:p["panel_id"]);panel_ids=[p["panel_id"] for p in panels];selected:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set();provenance:set[str]=set();required={f"{s}:{m}" for s in r["required_studies"] for m in r["required_modalities"]};present:set[str]=set()
    for p in panels:
        i=p["panel_id"]
        if p["study_id"] not in r["required_studies"] or p["modality"] not in r["required_modalities"]:omissions.add(f"{i}:outside-required-closure");continue
        present.add(f"{p['study_id']}:{p['modality']}")
        if p.get("local") is not True or p.get("aggregate_only") is not True:blocked.add(i);omissions.add(f"{i}:raw-data-locality")
        elif p["replay_identity"]!=r["replay_identity"]:unresolved.add(i);uncertainty.add(f"{i}:replay-identity")
        elif p["comparability_key"]!=r["comparability_key"]:unresolved.add(i);omissions.add(f"{i}:comparability")
        elif p.get("evidence_state")=="contradicted":blocked.add(i);negative.add(f"{i}:contradicted")
        elif p.get("evidence_state") not in {"proven","supported"}:unresolved.add(i);uncertainty.add(f"{i}:evidence-state")
        elif len(selected)<int(r["selected_panel_limit"]):selected.add(i);provenance.add(p["provenance_digest"])
        else:omissions.add(f"{i}:selection-limit")
    for pair in required - present:
        omissions.add(f"{pair}:missing")
    missing_studies={s for s in r["required_studies"] if not any(x.startswith(f"{s}:") for x in present)};missing_modalities={m for m in r["required_modalities"] if not any(x.endswith(f":{m}") for x in present)}
    if missing_studies:uncertainty.add("closure:study")
    if missing_modalities:uncertainty.add("closure:modality")
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"))
    if global_block:blocked.update(panel_ids);selected.clear();unresolved.clear();omissions.add("request:governance-or-locality-denied")
    so,uo,bo,ms,mm=(sorted(x) for x in (selected,unresolved,blocked,missing_studies,missing_modalities));disp="blocked" if global_block or (not so and not uo and not bo) else "unresolved" if uo or bo or ms or mm or omissions else "qualified";omissions.add("request:workspace-not-closed") if disp!="qualified" else None
    p={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"workspace_id":r["workspace_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"comparability_key":r["comparability_key"],"disposition":disp,"panel_order":panel_ids,"selected_panel_order":so,"unresolved_panel_order":uo,"blocked_panel_order":bo,"missing_study_order":ms,"missing_modality_order":mm,"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"view_order":sorted([f"overview:{r['workspace_id']}",f"provenance:{r['workspace_id']}",f"omissions:{r['workspace_id']}"]),"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};d=_hash(p);p["workspace_digest"]=d;p["artifact"]={"artifact_id":f"interactive-research-workspace-9:{r['workspace_id']}","content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":sorted(omissions),"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY};p["effect_receipts"]=sorted([f"view:research-workspace:{r['request_id']}",f"manage:local-capability:{r['request_id']}"] if disp=="qualified" else ["block:unsafe-release"]);out=InteractiveResearchWorkspace9(p);out.validate();return out
def idsResearchWorkbenchDigest(workspace:InteractiveResearchWorkspace9)->str:workspace.validate();return _hash(workspace.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","InteractiveResearchWorkspace9","research_workbench_manifest","compile_research_workbench","idsResearchWorkbenchDigest"]
