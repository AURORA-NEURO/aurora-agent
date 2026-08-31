"""Python parity for ``AFA-packs-P07-F25`` multimodal QC assurance."""
from __future__ import annotations
import hashlib,json,re
from dataclasses import dataclass
from typing import Any,Mapping,Sequence
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError
FEATURE_ID="AFA-packs-P07-F25"; CONTRACT_VERSION="packs-local-single-study-quality-control-assurance/1.0"; INPUT_SCHEMA="ResearchObject1@1"; OUTPUT_SCHEMA="QualityVerdict7@1"; CONTENT_TYPE="application/vnd.aurora.packs-quality-verdict-7+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return tuple(v)==tuple(sorted(set(v)))
@dataclass(frozen=True)
class PacksQualityVerdict7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(str(v.get(k,"")).strip() for k in ("request_id","study_id","requester","purpose","semantic_profile")) or int(v.get("checkpoint",0))<=0 or not v.get("observation_order") or not v.get("modality_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("quality identity, checkpoint, locality, observations, modalities, or effects are incomplete")
        fields=("observation_order","passed_order","failed_order","unknown_order","unmeasured_order","blocked_order","modality_order","passed_modality_order","missing_modality_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("quality ordering is not canonical")
        if set(v["observation_order"])!=set(v["passed_order"])|set(v["failed_order"])|set(v["unknown_order"])|set(v["unmeasured_order"])|set(v["blocked_order"]):raise ResearchContractError("quality observations do not partition")
        if set(v["modality_order"])!=set(v["passed_modality_order"])|set(v["missing_modality_order"]):raise ResearchContractError("quality modalities do not partition")
        a=v.get("artifact",{});ds=[v.get("replay_identity"),v.get("report_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]
        if not all(_digest(x) for x in ds) or a.get("content_type")!=CONTENT_TYPE or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_hash")!=v.get("report_digest"):raise ResearchContractError("quality artifact or digest is invalid")
        if any(not e.startswith("verify:packs-quality-verdict:") and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("quality effect is outside governed gate")
def packs_local_quality_control_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"packs","consumers":["benchmark curator","research program lead","research administrator"],"behavior":"evaluates a local single-study ResearchObject quality envelope against typed thresholds and modality closure","value":"prevents failed, missing, contradictory, or unmeasured QC evidence from silently entering a benchmark or research workflow","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["block:unsafe-release"],"permissions":["evaluate:capability-runs"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY}
def assure_packs_quality_control(request:Mapping[str,Any],observations:Sequence[Mapping[str,Any]])->PacksQualityVerdict7:
    if not all(str(request.get(k,"")).strip() for k in ("request_id","study_id","requester","purpose","semantic_profile")) or not request.get("required_modalities") or not 0<=int(request.get("minimum_pass_fraction_milli",-1))<=1000 or int(request.get("checkpoint",0))<=0 or int(request.get("budget_units",0))<=0 or request.get("boundary")!=PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _digest(request.get("replay_identity")) or not observations:raise ResearchContractError("quality identity, modalities, threshold, checkpoint, budget, replay, locality, observations, or boundary is invalid")
    rows=sorted((dict(x) for x in observations),key=lambda x:str(x.get("observation_id","")));ids=[str(x.get("observation_id","")) for x in rows]
    if len(set(ids))!=len(ids) or any(not x.get("observation_id") or not x.get("modality") or not x.get("metric_id") or not x.get("study_id") or not x.get("origin") or not x.get("semantic_profile") or not all(_digest(x.get(k)) for k in ("artifact_digest","provenance_digest","replay_identity")) for x in rows):raise ResearchContractError("observation identity, uniqueness, profiles, or digest is invalid")
    passed:set[str]=set();failed:set[str]=set();unknown:set[str]=set();unmeasured:set[str]=set();blocked:set[str]=set();mods={str(x["modality"]) for x in rows};passed_mods:set[str]=set();om:set[str]=set();unc:set[str]=set();neg:set[str]=set()
    for x in rows:
        oid=x["observation_id"];om.update(f"{oid}:{r}" for r in x.get("omission_reasons",[]));neg.add(f"{oid}:negative-result") if x.get("negative_result") else None;reasons=[]
        if x.get("study_id")!=request["study_id"]:reasons.append("study-mismatch")
        if x.get("semantic_profile")!=request["semantic_profile"]:reasons.append("semantic-profile-mismatch")
        if x.get("replay_identity")!=request["replay_identity"]:reasons.append("replay-identity-mismatch")
        if x.get("signed") is not True or x.get("permitted") is not True:reasons.append("authorization-missing")
        if x.get("raw_data_local") is not True or x.get("aggregate_only") is not True:reasons.append("locality-or-aggregate-only-failed")
        if x.get("evidence_state")=="contradicted":blocked.add(oid);neg.add(f"{oid}:contradicted")
        elif x.get("evidence_state")=="unknown":unknown.add(oid);unc.add(f"{oid}:unknown")
        elif x.get("evidence_state")=="unmeasured":unmeasured.add(oid);unc.add(f"{oid}:unmeasured")
        elif reasons:unknown.add(oid);unc.add(f"{oid}:unresolved")
        elif int(x.get("value_milli",0))<int(x.get("threshold_milli",0)):failed.add(oid);om.add(f"{oid}:threshold-failed")
        else:passed.add(oid);passed_mods.add(x["modality"])
    required=set(request["required_modalities"]);mods.update(required);missing=required-passed_mods;unc.add("modality:required-closure-incomplete") if missing else None;fraction=(len(passed)*1000)//len(rows);global_block=not all(request.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"));neg.add("request:policy-denied") if request.get("policy_allow") is not True else None;unc.add("request:protected-closure-incomplete") if request.get("protected_closure") is not True else None;unc.add("request:signed-approval-missing") if request.get("signed_approval") is not True else None
    disposition="blocked" if global_block or blocked else "unresolved" if fraction<int(request["minimum_pass_fraction_milli"]) or missing or failed or unknown or unmeasured else "qualified"
    if global_block:blocked.update(ids);passed.clear();failed.clear();unknown.clear();unmeasured.clear()
    om.add("request:quality-gates-incomplete") if disposition!="qualified" else None
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"study_id":request["study_id"],"requester":request["requester"],"purpose":request["purpose"],"semantic_profile":request["semantic_profile"],"checkpoint":int(request["checkpoint"]),"disposition":disposition,"observation_order":ids,"passed_order":sorted(passed),"failed_order":sorted(failed),"unknown_order":sorted(unknown),"unmeasured_order":sorted(unmeasured),"blocked_order":sorted(blocked),"modality_order":sorted(mods),"passed_modality_order":sorted(passed_mods),"missing_modality_order":sorted(missing),"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"pass_fraction_milli":fraction,"replay_identity":request["replay_identity"],"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);result={**payload,"report_digest":digest,"artifact":{"artifact_id":f"packs-quality-verdict-7:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance_digests":sorted({x["provenance_digest"] for x in rows}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":[f"verify:packs-quality-verdict:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True};receipt=PacksQualityVerdict7(result);receipt.validate();return receipt
def packsLocalQualityControlDigest(receipt:PacksQualityVerdict7)->str:receipt.validate();return _hash(receipt.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","PacksQualityVerdict7","packs_local_quality_control_manifest","assure_packs_quality_control","packsLocalQualityControlDigest"]
