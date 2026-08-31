"""Typed multimodal ingestion and harmonization for Worldgen P06 F01-F04."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
import re
from .research_contracts import PRECLINICAL_BOUNDARY,RESEARCH_CONTRACT_SCHEMA_VERSION,ResearchContractError,research_artifact_digest
CONTENT_TYPE="application/vnd.aurora.worldgen.multimodal-ingestion-receipt+json"; _HEX=re.compile(r"^[0-9a-f]{64}$")
@dataclass(frozen=True)
class ModalityObject:
    object_id:str; modality:str; dimensions:tuple[int,...]; unit:str; ontology:str; quality_milli:int; evidence_digest:str; provenance_digest:str; artifact_digest:str; replay_identity:str; semantic_loss:tuple[str,...]=(); available:bool=True; raw_data_local:bool=True; negative_result:bool=False; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class MultimodalIngestionRequest:
    bundle_id:str; consumer:str; required_modality_order:tuple[str,...]; object_order:tuple[str,...]; objects:tuple[ModalityObject,...]; minimum_quality_milli:int; replay_identity:str; policy_allow:bool=True; protected_closure:bool=True; federation_approved:bool=False; raw_data_local:bool=True; aggregate_only:bool=True; boundary:str=PRECLINICAL_BOUNDARY
@dataclass(frozen=True)
class MultimodalIngestionReceipt:
    value:dict[str,Any]
    def validate(self)->None:
        v,a=self.value,self.value.get("artifact",{})
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and a.get("raw_objects") is False and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("bundle_id") and v.get("object_order") and v.get("effect_receipts") and all(_HEX.fullmatch(v.get(k,"")) for k in ("replay_identity","ingestion_digest")) and a.get("content_hash")==v.get("ingestion_digest")): raise ResearchContractError("multimodal identity, objects, locality, digests, or effects are incomplete")
        for key in ("required_modality_order","object_order","harmonized_order","missing_order","stale_order","blocked_order","unknown_order","omitted_order","semantic_loss_order","omissions","uncertainty","negative_evidence","effect_receipts"):
            vals=tuple(v.get(key,()))
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("multimodal vectors are not canonical")
        ids=set(v["object_order"]); parts=set(v.get("harmonized_order",()))|set(v.get("missing_order",()))|set(v.get("stale_order",()))|set(v.get("blocked_order",()))|set(v.get("unknown_order",()))|set(v.get("omitted_order",()))
        if len(ids)!=len(v["object_order"]) or parts!=ids or {x["object_id"] for x in v.get("objects",())}!=set(v.get("harmonized_order",())): raise ResearchContractError("multimodal object states do not partition")
    def digest(self)->str:self.validate();return research_artifact_digest(self.value)
def manifest(*,feature_id:str,contract_version:str,input_schema:str,scale:str,autonomy_tier:str)->dict[str,Any]:
    return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["bioinformatician","imaging core scientist","benchmark curator","preclinical neuroscientist"],"behavior":f"harmonize typed imaging and omics research objects for {scale}","value":"makes coordinates, units, ontology, quality, semantic loss, and local residency explicit before multimodal analysis","input_schema":input_schema,"output_schema":"HarmonizedResearchObject1@1","effects":["ingest:worldgen-multimodal","block:unsafe-release"],"permissions":["ingest:local-modality-bundle"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY,"contract_version":contract_version}
def ingest(request:MultimodalIngestionRequest,*,feature_id:str,contract_version:str,scale:str,require_federation:bool=False)->MultimodalIngestionReceipt:
    if not(request.bundle_id.strip() and request.consumer.strip() and request.required_modality_order and request.object_order and tuple(request.required_modality_order)==tuple(sorted(set(request.required_modality_order))) and tuple(request.object_order)==tuple(sorted(set(request.object_order))) and request.boundary==PRECLINICAL_BOUNDARY and request.raw_data_local and request.aggregate_only and _HEX.fullmatch(request.replay_identity)): raise ResearchContractError("multimodal identity, objects, locality, boundary, ordering, or replay is invalid")
    if require_federation and not request.federation_approved: raise ResearchContractError("multimodal federation approval is required")
    ids=set(request.object_order); by={}
    for x in request.objects:
        if x.object_id not in ids or x.boundary!=PRECLINICAL_BOUNDARY or not x.raw_data_local or x.replay_identity!=request.replay_identity or not all(_HEX.fullmatch(getattr(x,k,"")) for k in ("evidence_digest","provenance_digest","artifact_digest","replay_identity")): raise ResearchContractError("modality object identity, provenance, locality, or replay is invalid")
        if x.object_id in by: raise ResearchContractError("duplicate modality object")
        by[x.object_id]=x
    required=set(request.required_modality_order); good=[]; missing=set(); stale=set(); blocked=set(); unknown=set(); omitted=set(); loss=set(); omissions=set(); uncertainty=set(); negative=set()
    for oid in sorted(ids):
        x=by.get(oid)
        if x is None: missing.add(oid); omissions.add(f"object:{oid}:missing")
        elif x.negative_result: unknown.add(oid); negative.add(f"object:{oid}:negative-result-retained")
        elif not request.policy_allow or not request.protected_closure: blocked.add(oid); omissions.add(f"object:{oid}:policy-or-closure-blocked")
        elif not x.available: stale.add(oid); uncertainty.add(f"object:{oid}:unavailable-or-stale")
        elif x.quality_milli<request.minimum_quality_milli or x.modality not in required: unknown.add(oid); uncertainty.add(f"object:{oid}:quality-or-modality-below-threshold")
        else: good.append(x); loss.update(f"{oid}:{item}" for item in x.semantic_loss)
    selected=[{"object_id":x.object_id,"modality":x.modality,"dimensions":list(x.dimensions),"unit":x.unit,"ontology":x.ontology,"quality_milli":x.quality_milli,"artifact_digest":x.artifact_digest} for x in sorted(good,key=lambda x:x.object_id)]; selected_ids={x["object_id"] for x in selected}; omitted|=ids-selected_ids-missing-stale-blocked-unknown; omissions|={f"object:{x}:not-selected" for x in omitted if x not in {m for m in missing|stale|blocked|unknown}}
    authority=request.policy_allow and request.protected_closure and (not require_federation or request.federation_approved); disposition="blocked" if not authority else "unknown" if not selected else "qualified" if len(selected)==len(request.object_order) and not omissions and not uncertainty and not negative and not loss else "partial"; effects=["block:unsafe-release"] if disposition=="blocked" else [f"ingest:worldgen-multimodal:{request.bundle_id}"]
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"bundle_id":request.bundle_id,"consumer":request.consumer,"scale":scale,"disposition":disposition,"required_modality_order":sorted(required),"object_order":sorted(ids),"harmonized_order":sorted(selected_ids),"missing_order":sorted(missing),"stale_order":sorted(stale),"blocked_order":sorted(blocked),"unknown_order":sorted(unknown),"omitted_order":sorted(omitted),"semantic_loss_order":sorted(loss),"objects":selected,"omissions":sorted(omissions),"uncertainty":sorted(uncertainty),"negative_evidence":sorted(negative),"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY,"raw_objects":False}; d=research_artifact_digest(payload); payload["ingestion_digest"]=d; payload["artifact"]={"artifact_id":f"worldgen-harmonized-bundle:{request.bundle_id}","content_type":CONTENT_TYPE,"content_hash":d,"raw_objects":False,"boundary":PRECLINICAL_BOUNDARY}; receipt=MultimodalIngestionReceipt(payload); receipt.validate(); return receipt
__all__=["CONTENT_TYPE","ModalityObject","MultimodalIngestionRequest","MultimodalIngestionReceipt","manifest","ingest"]
