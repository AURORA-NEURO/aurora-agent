"""Typed retrieval-contract compatibility compiler for Worldgen P02 F05–F08."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_retrieval_support import RetrievalCandidate

_HEX = re.compile(r"^[0-9a-f]{64}$")
OUTPUT_SCHEMA = "EvidenceSynthesis2@1"
CONTENT_TYPE = "application/vnd.aurora.worldgen.retrieval-contract-receipt+json"

@dataclass(frozen=True)
class RetrievalContractRequest:
    request_id: str; consumer: str; scope: str; semantic_profile: str; input_schema: str; output_schema: str
    required_candidate_order: tuple[str, ...]; candidates: tuple[RetrievalCandidate, ...]; replay_identity: str
    policy_allow: bool = True; protected_closure: bool = True; raw_data_local: bool = True; aggregate_only: bool = True; boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class RetrievalContractReceipt:
    value: dict[str, Any]
    def validate(self, *, feature_id: str, contract_version: str) -> None:
        v=self.value; a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=contract_version or v.get("feature_id")!=feature_id or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("content_type")!=CONTENT_TYPE or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k),str) and v.get(k).strip() for k in ("request_id","consumer","scope","semantic_profile")) or v.get("output_schema")!=OUTPUT_SCHEMA or not v.get("candidate_order") or not v.get("effect_receipts"):
            raise ResearchContractError("worldgen retrieval contract identity, schemas, locality, or effects are incomplete")
        for key in ("candidate_order","compatible_order","unresolved_order","blocked_order","omitted_order","negative_evidence_order","migration_order","semantic_loss_order","effect_receipts"):
            vals=tuple(v.get(key,()));
            if vals!=tuple(sorted(set(vals))): raise ResearchContractError("worldgen retrieval contract ordering is not canonical")
        ids=set(v["candidate_order"]); parts=list(v.get("compatible_order",()))+list(v.get("unresolved_order",()))+list(v.get("blocked_order",()))+list(v.get("omitted_order",()))
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or len(set(parts))!=len(parts) or set(parts)!=ids: raise ResearchContractError("worldgen retrieval contract candidate states do not partition")
        for digest in (v.get("replay_identity"),v.get("contract_digest"),a.get("content_hash")):
            if not isinstance(digest,str) or not _HEX.fullmatch(digest): raise ResearchContractError("worldgen retrieval contract digest is invalid")
        if a.get("content_hash")!=v.get("contract_digest"): raise ResearchContractError("worldgen retrieval contract artifact digest is inconsistent")
    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id,contract_version=contract_version); return _digest(self.value)

def _digest(value: Any)->str: return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str)->dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["benchmark curator","research program lead","preclinical neuroscientist","bioinformatician"],"behavior":f"compile a typed retrieval contract for {scale} with deterministic compatibility and semantic-loss receipts","value":"prevents schema drift and silent defaults before evidence synthesis","input_schema":input_schema,"output_schema":OUTPUT_SCHEMA,"effects":["none:contract-validation"],"permissions":["read:local-research-artifacts"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":PRECLINICAL_BOUNDARY}
def compile_contract(request: RetrievalContractRequest, *, feature_id: str, contract_version: str, expected_input_schema: str)->RetrievalContractReceipt:
    if not all(isinstance(x,str) and x.strip() for x in (request.request_id,request.consumer,request.scope,request.semantic_profile,request.input_schema,request.output_schema)) or not request.required_candidate_order or not request.candidates or request.output_schema!=OUTPUT_SCHEMA or request.boundary!=PRECLINICAL_BOUNDARY or not request.raw_data_local or not request.aggregate_only or not _HEX.fullmatch(request.replay_identity): raise ResearchContractError("worldgen retrieval contract identity, schemas, candidates, replay, locality, or boundary is invalid")
    ids=sorted({c.candidate_id for c in request.candidates})
    if len(ids)!=len(request.candidates) or any(i not in ids for i in request.required_candidate_order): raise ResearchContractError("worldgen retrieval contract candidate identifiers do not match the declared set")
    compatible:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); omitted:set[str]=set(); negative:set[str]=set(); migration:set[str]=set(); semantic_loss:set[str]=set(); schema_break=request.input_schema!=expected_input_schema
    for c in request.candidates:
        if c.negative_result: negative.add(f"candidate:{c.candidate_id}:negative-result-retained")
        if c.candidate_id not in request.required_candidate_order: omitted.add(f"candidate:{c.candidate_id}:not-required"); semantic_loss.add(f"candidate:{c.candidate_id}:outside-required-closure"); continue
        if schema_break: migration.add(f"request:input-schema:{request.input_schema}->{expected_input_schema}"); semantic_loss.add(f"candidate:{c.candidate_id}:schema-version-unresolved"); unresolved.add(c.candidate_id)
        elif not request.policy_allow or not request.protected_closure or not c.permitted: blocked.add(c.candidate_id)
        elif c.evidence_state=="supported" and c.comparable and c.replay_identity==request.replay_identity: compatible.add(c.candidate_id)
        elif c.evidence_state in {"unknown","unmeasured","speculative"}: unresolved.add(c.candidate_id)
        else: blocked.add(c.candidate_id)
        if not c.comparable: semantic_loss.add(f"candidate:{c.candidate_id}:incomparable")
        if c.replay_identity!=request.replay_identity: semantic_loss.add(f"candidate:{c.candidate_id}:replay-mismatch")
    for required in request.required_candidate_order:
        if required not in ids: omitted.add(f"candidate:{required}:missing"); semantic_loss.add(f"candidate:{required}:missing-required")
    if not request.policy_allow: semantic_loss.add("request:policy-denied")
    if not request.protected_closure: semantic_loss.add("request:protected-closure-incomplete")
    compatibility="breaking" if schema_break else "additive_migration" if migration else "compatible"
    disposition="blocked" if schema_break or not request.policy_allow or not request.protected_closure else "unknown" if not compatible else "compatible" if not unresolved and not blocked and not omitted and not semantic_loss else "partial"
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"consumer":request.consumer,"scope":request.scope,"semantic_profile":request.semantic_profile,"input_schema":request.input_schema,"output_schema":request.output_schema,"compatibility":compatibility,"disposition":disposition,"candidate_order":ids,"compatible_order":sorted(compatible),"unresolved_order":sorted(unresolved),"blocked_order":sorted(blocked),"omitted_order":sorted(omitted),"negative_evidence_order":sorted(negative),"migration_order":sorted(migration),"semantic_loss_order":sorted(semantic_loss),"replay_identity":request.replay_identity,"effect_receipts":["none:contract-validation"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}
    digest=_digest(payload); payload["contract_digest"]=digest; payload["artifact"]={"artifact_id":f"retrieval-contract:{request.request_id}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":sorted(semantic_loss),"boundary":PRECLINICAL_BOUNDARY}
    receipt=RetrievalContractReceipt(payload); receipt.validate(feature_id=feature_id,contract_version=contract_version); return receipt

__all__=["RetrievalCandidate","RetrievalContractRequest","RetrievalContractReceipt","compile_contract","manifest"]
