"""Cross-language contract for ``AFA-lens-P18-F10``."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-lens-P18-F10"; CONTRACT_VERSION="lens-multimodal-provenance-signing-research-copilot/1.0"; INPUT_SCHEMA="ArtifactAndDerivation2@1"; OUTPUT_SCHEMA="SignedProvenanceEnvelope3@1"; CONTENT_TYPE="application/vnd.aurora.lens-signed-provenance-envelope-3+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _ordered(v:list[str])->bool:return v==sorted(set(v))
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _partition(u:list[str],p:list[list[str]],m:str)->None:
    f=sum(p,[])
    if len(set(u))!=len(u) or len(f)!=len(set(f)) or set(f)!=set(u):raise ResearchContractError(m)

@dataclass(frozen=True)
class SignedProvenanceEnvelope3:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{});text=("request_id","requester","purpose","scope","semantic_profile")
        if not(v.get("schema_version")==RESEARCH_CONTRACT_SCHEMA_VERSION and v.get("contract_version")==CONTRACT_VERSION and v.get("feature_id")==FEATURE_ID and v.get("boundary")==PRECLINICAL_BOUNDARY and a.get("boundary")==PRECLINICAL_BOUNDARY and a.get("content_type")==CONTENT_TYPE and v.get("raw_data_local") is True and v.get("aggregate_only") is True and v.get("autonomy_tier")=="a2" and all(isinstance(v.get(k),str) and v[k].strip() for k in text) and v.get("artifact_order") and v.get("study_order") and v.get("modality_order") and v.get("signer_order") and v.get("effect_receipts") and v.get("disposition") in {"qualified","unresolved","blocked"}):raise ResearchContractError("provenance identity, axes, locality, autonomy, or effects are incomplete")
        fields=("artifact_order","selected_artifact_order","unresolved_artifact_order","blocked_artifact_order","missing_artifact_order","study_order","modality_order","selected_study_order","selected_modality_order","missing_study_order","missing_modality_order","signer_order","selected_signer_order","missing_signer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts")
        if any(not _ordered(list(v.get(k,[]))) for k in fields):raise ResearchContractError("provenance ordering is not canonical")
        _partition(v["artifact_order"],[v["selected_artifact_order"],v["unresolved_artifact_order"],v["blocked_artifact_order"],v["missing_artifact_order"]],"artifact states do not form a complete partition")
        for d in ("replay_identity","envelope_digest"):
            if not _digest(v.get(d)):raise ResearchContractError("provenance digest is invalid")
        if a.get("content_hash")!=v.get("envelope_digest"):raise ResearchContractError("provenance artifact metadata is inconsistent")
        if any(not(e.startswith("invoke:declared-tools:") or e=="block:unsafe-release") for e in v["effect_receipts"]):raise ResearchContractError("provenance effect is outside declared-tool gate")
        if v["disposition"]=="qualified" and v["effect_receipts"]!=[f"invoke:declared-tools:{v['request_id']}"]:raise ResearchContractError("qualified provenance effect is invalid")
        if v["disposition"]!="qualified" and v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("non-qualified provenance must block")
    def digest(self)->str:self.validate();return _hash(self.value)

def provenance_signing_copilot_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"lens","consumers":["bioinformatician","provenance reviewer","research object publisher"],"behavior":"compiles multimodal artifact and derivation attestations into a deterministic signed-provenance envelope without authenticating keys or moving raw data","value":"makes lineage, signing coverage, replay identity, omissions, and negative results auditable before bounded tool invocation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["read_local_data","write_local_artifact"],"permissions":["invoke:declared-tools"],"authority_requirements":[{"role":"provenance reviewer","reason":"declared-tool invocation requires explicit lineage review"}],"autonomy_tier":"a2","boundary":PRECLINICAL_BOUNDARY}

def _validate_request(q:Mapping[str,Any])->None:
    text=("request_id","requester","purpose","scope","semantic_profile")
    if not(q.get("schema_version")==INPUT_SCHEMA and all(isinstance(q.get(k),str) and q[k].strip() for k in text) and q.get("required_studies") and q.get("required_modalities") and q.get("artifacts") and _digest(q.get("replay_identity")) and all(isinstance(q.get(k),bool) for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only")) and q.get("raw_data_local") is True and q.get("aggregate_only") is True and q.get("boundary")==PRECLINICAL_BOUNDARY):raise ResearchContractError("provenance request identity, closure, replay, locality, or bounds are invalid")
    ids=[x.get("artifact_id") for x in q["artifacts"]]
    if any(not isinstance(x,str) or not x.strip() for x in ids) or len(ids)!=len(set(ids)):raise ResearchContractError("artifact identifiers must be present and unique")

def compile_provenance_envelope(request:Mapping[str,Any])->SignedProvenanceEnvelope3:
    _validate_request(request);rows=sorted((dict(x) for x in request["artifacts"]),key=lambda x:(x["study_id"],x["modality"],x["artifact_id"]));artifact=[x["artifact_id"] for x in rows];selected=[];unresolved=[];blocked=[];missing=[];omission=set();uncertainty=set();negative=set()
    for x in rows:
        i=x["artifact_id"]
        if not x.get("source_digest") or not x.get("provenance_digest") or not x.get("signer_key_digest"):missing.append(i);omission.add(f"{i}:lineage-or-signer-missing")
        elif not x.get("scope_compatible",False) or not x.get("policy_allowed",False):blocked.append(i);omission.add(f"{i}:scope-or-policy-denied")
        elif x.get("evidence_state")=="contradicted":blocked.append(i);uncertainty.add(f"{i}:contradicted")
        elif x.get("evidence_state") in {"unknown","speculative"} or x.get("replay_identity")!=request["replay_identity"]:unresolved.append(i);uncertainty.add(f"{i}:unknown-or-replay-mismatch")
        else:selected.append(i);negative.add(f"{i}:negative-result") if x.get("negative_result") is True else None;omission.update(f"{i}:{e}" for e in x.get("omissions",[]))
    studies=sorted({x["study_id"] for x in rows}|set(request["required_studies"]));modalities=sorted({x["modality"] for x in rows}|set(request["required_modalities"]));present_s={x["study_id"] for x in rows};present_m={x["modality"] for x in rows};missing_s=sorted(set(request["required_studies"])-present_s);missing_m=sorted(set(request["required_modalities"])-present_m);omission.update(f"study:{x}:missing" for x in missing_s);omission.update(f"modality:{x}:missing" for x in missing_m);omission.update(f"request:adversarial:{x}" for x in request.get("adversarial_events",[]));signers=sorted({x["signer_id"] for x in rows});selected_set=set(selected);selected_s=sorted({x["study_id"] for x in rows if x["artifact_id"] in selected_set});selected_m=sorted({x["modality"] for x in rows if x["artifact_id"] in selected_set});selected_signers=sorted({x["signer_id"] for x in rows if x["artifact_id"] in selected_set});missing_signers=sorted(set(signers)-set(selected_signers));global_open=all(request.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only")) and not request.get("adversarial_events");disposition="blocked" if not global_open or blocked or missing_s or missing_m else ("unresolved" if missing or unresolved else "qualified");effects=[f"invoke:declared-tools:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-release"]
    payload={"schema_version":OUTPUT_SCHEMA,"request_id":request["request_id"],"artifact_order":artifact,"selected_artifact_order":selected,"unresolved_artifact_order":unresolved,"blocked_artifact_order":blocked,"missing_artifact_order":missing,"study_order":studies,"modality_order":modalities,"disposition":disposition,"replay_identity":request["replay_identity"]};digest=_hash(payload);value={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"requester":request["requester"],"purpose":request["purpose"],"scope":request["scope"],"semantic_profile":request["semantic_profile"],"disposition":disposition,"artifact_order":artifact,"selected_artifact_order":selected,"unresolved_artifact_order":unresolved,"blocked_artifact_order":blocked,"missing_artifact_order":missing,"study_order":studies,"modality_order":modalities,"selected_study_order":selected_s,"selected_modality_order":selected_m,"missing_study_order":missing_s,"missing_modality_order":missing_m,"signer_order":signers,"selected_signer_order":selected_signers,"missing_signer_order":missing_signers,"omission_order":sorted(omission),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"replay_identity":request["replay_identity"],"envelope_digest":digest,"artifact":{"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"signed-provenance:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":[],"provenance":[],"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":effects,"raw_data_local":True,"aggregate_only":True,"autonomy_tier":"a2","boundary":PRECLINICAL_BOUNDARY};r=SignedProvenanceEnvelope3(value);r.validate();return r

__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","SignedProvenanceEnvelope3","provenance_signing_copilot_manifest","compile_provenance_envelope"]
