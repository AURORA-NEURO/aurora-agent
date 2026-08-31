"""Python parity for ``AFA-ids-P06-F09`` multimodal ingestion copilot."""
from __future__ import annotations
import hashlib,json,re
from dataclasses import dataclass
from typing import Any,Mapping,Sequence
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-ids-P06-F09"; CONTRACT_VERSION="ids-local-single-study-multimodal-ingestion-research-copilot/1.0"; INPUT_SCHEMA="MultimodalIngestionRequest4@1"; OUTPUT_SCHEMA="HarmonizedResearchObject8@1"; CONTENT_TYPE="application/vnd.aurora.harmonized-research-object-8+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return tuple(v)==tuple(sorted(set(v)))
@dataclass(frozen=True)
class HarmonizedResearchObject8:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","study_id","requester","purpose","semantic_profile")) or int(v.get("checkpoint",0))<=0 or not v.get("observation_order") or not v.get("modality_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("ingestion identity, checkpoint, locality, observations, modalities, or effects are incomplete")
        fields=("observation_order","selected_order","unresolved_order","blocked_order","modality_order","selected_modality_order","missing_modality_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("ingestion ordering is not canonical")
        if set(v["observation_order"])!=set(v["selected_order"])|set(v["unresolved_order"])|set(v["blocked_order"]):raise ResearchContractError("observation states do not partition")
        if set(v["modality_order"])!=set(v["selected_modality_order"])|set(v["missing_modality_order"]):raise ResearchContractError("modality states do not partition")
        a=v.get("artifact",{});ds=[v.get("replay_identity"),v.get("object_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]
        if not all(_digest(x) for x in ds) or len(v["selected_order"])!=len(v.get("quality_scores_milli",[])) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("object_digest"):raise ResearchContractError("ingestion artifact, digest, or quality cardinality is invalid")
        if any(not e.startswith("manage:local-capability:") and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("ingestion effect is outside governed gate")
def multimodal_ingestion_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["computational biologist","imaging scientist","omics scientist"],"behavior":"validates bounded local modality manifests and quality summaries into a harmonized research-object receipt","value":"prevents incompatible, incomplete, or unauthorized modalities from silently entering a preclinical study workflow","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def operate_multimodal_ingestion(request:Mapping[str,Any],observations:Sequence[Mapping[str,Any]])->HarmonizedResearchObject8:
    if not all(str(request.get(k,"")).strip() for k in ("request_id","study_id","requester","purpose","semantic_profile")) or not request.get("required_modalities") or int(request.get("minimum_quality_milli",0))<0 or int(request.get("checkpoint",0))<=0 or int(request.get("budget_units",0))<=0 or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")) or not observations:raise ResearchContractError("ingestion identity, modalities, checkpoint, budget, replay, locality, observations, or boundary is invalid")
    rows=sorted((dict(x) for x in observations),key=lambda x:(-int(x.get("quality_milli",0)),str(x.get("observation_id",""))));ids=[str(x.get("observation_id","")) for x in rows]
    if len(set(ids))!=len(ids) or any(not x.get("observation_id") or not x.get("modality") or not x.get("study_id") or not x.get("origin") or not x.get("schema_version") or not x.get("semantic_profile") or not x.get("unit_profile") or not x.get("coordinate_profile") or not all(_digest(x.get(k)) for k in ("artifact_digest","provenance_digest","replay_identity")) for x in rows):raise ResearchContractError("observation identity, uniqueness, profiles, or digest is invalid")
    selected:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();mods={str(x["modality"]) for x in rows};selected_mods:set[str]=set();om:set[str]=set();unc:set[str]=set();neg:set[str]=set();scores:list[int]=[]
    for x in rows:
        oid=x["observation_id"];om.update(f"{oid}:{r}" for r in x.get("omission_reasons",[]));neg.add(f"{oid}:negative-result") if x.get("negative_result") else None;reasons=[]
        if x.get("study_id")!=request["study_id"]:reasons.append("study-mismatch")
        if x.get("semantic_profile")!=request["semantic_profile"]:reasons.append("semantic-profile-mismatch")
        if int(x.get("quality_milli",0))<int(request["minimum_quality_milli"]):reasons.append("quality-threshold-failed");om.add(f"{oid}:quality-threshold")
        if x.get("replay_identity")!=request["replay_identity"]:reasons.append("replay-identity-mismatch")
        if x.get("signed") is not True or x.get("permitted") is not True:reasons.append("authorization-missing")
        if x.get("raw_data_local") is not True or x.get("aggregate_only") is not True:reasons.append("locality-or-aggregate-only-failed")
        if x.get("evidence_state")=="contradicted":blocked.add(oid);neg.add(f"{oid}:contradicted")
        elif x.get("evidence_state") not in {"proven","supported"} or reasons:unresolved.add(oid);unc.add(f"{oid}:unresolved")
        else:selected.add(oid);selected_mods.add(x["modality"]);scores.append(int(x.get("quality_milli",0)))
    required=set(request["required_modalities"]);mods.update(required);missing=required-selected_mods
    if missing:unc.add("modality:required-closure-incomplete")
    global_block=not all(request.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"));neg.add("request:policy-denied") if request.get("policy_allow") is not True else None;unc.add("request:protected-closure-incomplete") if request.get("protected_closure") is not True else None;unc.add("request:signed-approval-missing") if request.get("signed_approval") is not True else None
    disposition="blocked" if global_block or blocked else "unresolved" if not selected or missing else "qualified"
    if global_block:blocked.update(ids);selected.clear();unresolved.clear();scores.clear()
    om.add("request:ingestion-gates-incomplete") if disposition!="qualified" else None
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"study_id":request["study_id"],"requester":request["requester"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"checkpoint":int(request["checkpoint"]),"disposition":disposition,"observation_order":ids,"selected_order":sorted(selected),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"modality_order":sorted(mods),"selected_modality_order":sorted(selected_mods),"missing_modality_order":sorted(missing),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"quality_scores_milli":scores,"replay_identity":request["replay_identity"],"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);result={**payload,"object_digest":digest,"artifact":{"artifact_id":f"harmonized-research-object-8:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"manage:local-capability:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True};receipt=HarmonizedResearchObject8(result);receipt.validate();return receipt
def idsMultimodalIngestionDigest(receipt:HarmonizedResearchObject8)->str:receipt.validate();return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","HarmonizedResearchObject8","multimodal_ingestion_manifest","operate_multimodal_ingestion","idsMultimodalIngestionDigest"]
