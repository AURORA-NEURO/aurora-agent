"""Python parity for ``AFA-worldgen-P06-F28`` modality-ingestion assurance."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-worldgen-P06-F28"; CONTRACT_VERSION="worldgen-federated-continual-multimodal-ingestion-assurance/1.0"; INPUT_SCHEMA="WorldgenMultimodalIngestionRequest8@1"; OUTPUT_SCHEMA="WorldgenHarmonizedIngestionReceipt10@1"; CONTENT_TYPE="application/vnd.aurora.worldgen-harmonized-ingestion-receipt-10+json"; MAX_OBSERVATIONS=16_384
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return values==sorted(set(values))
@dataclass(frozen=True)
class WorldgenHarmonizedIngestionReceipt10:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","world_id","purpose","semantic_profile")) or not v.get("required_modality_order") or not v.get("observation_order") or not v.get("effect_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("ingestion identity, modalities, observations, effects, locality, or disposition is incomplete")
        for k in ("required_modality_order","observation_order","selected_observation_order","unresolved_observation_order","blocked_observation_order","modality_order","missing_modality_order","quality_failed_order","contradiction_order","stale_order","evidence_order","omission_order","uncertainty_order","negative_evidence_order","effect_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("ingestion ordering is not canonical")
        ids=set(v["observation_order"]);parts=v["selected_observation_order"]+v["unresolved_observation_order"]+v["blocked_observation_order"]
        if len(ids)!=len(v["observation_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("ingestion states do not partition")
        if not _digest(v.get("replay_identity")) or not _digest(v.get("harmonization_digest")) or a.get("content_hash")!=v.get("harmonization_digest") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(d) for d in a.get("provenance_digests",[])):raise ResearchContractError("ingestion digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:worldgen-harmonized-digests:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("effect is outside governed ingestion gate")
    def digest(self)->str:self.validate();return _hash(self.to_dict())
def multimodal_ingestion_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"worldgen","consumers":["preclinical neuroscientist","multimodal ingestion operator","benchmark curator"],"behavior":"qualify synthetic-world multimodal observation summaries with deterministic quality, modality closure, semantic-profile, evidence, replay, provenance, policy, and locality gates","value":"prevents incomplete or unmeasured worldgen modality bundles from entering downstream research workflows","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:worldgen-harmonized-digests","manage:local-capability"],"permissions":["read:local-modality-summaries","request:worldgen-multimodal-ingestion"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","world_id","purpose","semantic_profile")) or not r.get("required_modality_order") or not r.get("observations") or len(r["observations"])>MAX_OBSERVATIONS or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("ingestion identity, modality requirements, observation bound, replay, locality, or boundary is invalid")
    req=r["required_modality_order"]
    if len(set(req))!=len(req) or any(not isinstance(x,str) or not x.strip() for x in req):raise ResearchContractError("required modalities must be unique and non-empty")
    ids:set[str]=set()
    for o in r["observations"]:
        if not isinstance(o,Mapping) or not isinstance(o.get("observation_id"),str) or not o["observation_id"].strip() or o["observation_id"] in ids or not isinstance(o.get("world_id"),str) or not o["world_id"].strip() or not isinstance(o.get("modality"),str) or not o["modality"].strip() or not isinstance(o.get("semantic_profile"),str) or not o["semantic_profile"].strip() or not isinstance(o.get("quality_milli"),int) or o["quality_milli"]<0 or not _digest(o.get("artifact_digest")) or not _digest(o.get("provenance_digest")) or not _digest(o.get("replay_identity")) or o.get("local") is not True or o.get("aggregate_only") is not True or o.get("evidence_state") not in {"proven","supported","unknown","unmeasured","contradicted"}:raise ResearchContractError(f"observation {o.get('observation_id','')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(o["observation_id"])
def assure_worldgen_multimodal_ingestion(r:Mapping[str,Any])->WorldgenHarmonizedIngestionReceipt10:
    _validate_request(r); obs=sorted((dict(o) for o in r["observations"]),key=lambda o:o["observation_id"]); order=[o["observation_id"] for o in obs]; selected:set[str]=set();unresolved:set[str]=set();blocked:set[str]=set();modalities:set[str]=set();missing:set[str]=set();quality:set[str]=set();contradiction:set[str]=set();evidence:set[str]=set();omissions:set[str]=set();uncertainty:set[str]=set();negative:set[str]=set();provenance:set[str]=set()
    for o in obs:
        modalities.add(o["modality"]);provenance.add(o["provenance_digest"]);oid=o["observation_id"]
        if o["world_id"]!=r["world_id"]:unresolved.add(oid);uncertainty.add(f"{oid}:world-id")
        elif o["semantic_profile"]!=r["semantic_profile"]:unresolved.add(oid);uncertainty.add(f"{oid}:semantic-profile")
        elif o["replay_identity"]!=r["replay_identity"]:unresolved.add(oid);uncertainty.add(f"{oid}:replay-identity")
        elif o["quality_milli"]<r["quality_floor_milli"]:unresolved.add(oid);quality.add(oid);negative.add(f"{oid}:quality-below-floor")
        elif o["evidence_state"]=="contradicted":blocked.add(oid);contradiction.add(oid);negative.add(f"{oid}:contradicted")
        elif o["evidence_state"] not in {"proven","supported"}:unresolved.add(oid);evidence.add(oid);uncertainty.add(f"{oid}:evidence-state")
        else:selected.add(oid)
    for modality in r["required_modality_order"]:
        if modality not in modalities:missing.add(modality);omissions.add(f"modality:{modality}:missing");negative.add(f"modality:{modality}:no-observation")
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"))
    if global_block:blocked.update(order);selected.clear();unresolved.clear();omissions.add("request:governance-or-locality-denied")
    so=sorted(selected);uo=sorted(unresolved);bo=sorted(blocked);disp="blocked" if global_block or (not so and not uo) else ("unresolved" if bo or uo or missing else "qualified")
    if disp!="qualified":omissions.add("request:multimodal-ingestion-not-closed")
    effects=sorted(["exchange:worldgen-harmonized-digests","manage:local-capability"] if disp=="qualified" else ["block:unsafe-release"])
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"world_id":r["world_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"disposition":disp,"required_modality_order":r["required_modality_order"],"observation_order":order,"selected_observation_order":so,"unresolved_observation_order":uo,"blocked_observation_order":bo,"modality_order":sorted(modalities),"missing_modality_order":sorted(missing),"quality_failed_order":sorted(quality),"contradiction_order":sorted(contradiction),"stale_order":[],"evidence_order":sorted(evidence),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"effect_order":effects,"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY};digest=_hash(payload);v=dict(payload);v["harmonization_digest"]=digest;v["artifact"]={"artifact_id":f"worldgen-harmonized-ingestion-receipt-10:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY};v["effect_receipts"]=sorted(e if e=="block:unsafe-release" else f"{e}:{r['request_id']}" for e in effects);receipt=WorldgenHarmonizedIngestionReceipt10(v);receipt.validate();return receipt
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","WorldgenHarmonizedIngestionReceipt10","multimodal_ingestion_assurance_manifest","assure_worldgen_multimodal_ingestion"]
