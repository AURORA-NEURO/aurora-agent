"""Python parity for ``AFA-hubapi-P09-F28`` experiment-design assurance."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-hubapi-P09-F28"; CONTRACT_VERSION="hubapi-federated-continual-experiment-design-assurance-harness/1.0"; INPUT_SCHEMA="ExperimentObjective4@1"; OUTPUT_SCHEMA="ExecutableExperimentDesign7@1"; CONTENT_TYPE="application/vnd.aurora.hubapi-experiment-design-assurance-7+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class ExecutableExperimentDesign7:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value;a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("disposition") not in {"qualified","partial","blocked"} or not v.get("candidate_order") or not v.get("peer_order") or not v.get("effect_receipts") or not all(str(v.get(k,"")).strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile")):raise ResearchContractError("experiment-design assurance identity, locality, candidates, peers, or effects are incomplete")
        for k in ("candidate_order","qualified_order","unresolved_order","blocked_order","missing_modality_order","missing_control_order","peer_order","qualified_peer_order","missing_peer_order","omission_order","uncertainty_order","negative_evidence_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("experiment-design ordering is not canonical")
        ids=set(v["candidate_order"]);parts=[*v["qualified_order"],*v["unresolved_order"],*v["blocked_order"]];peers=set(v["peer_order"]);pp=[*v["qualified_peer_order"],*v["missing_peer_order"]]
        if len(ids)!=len(v["candidate_order"]) or len(parts)!=len(ids) or set(parts)!=ids or len(peers)!=len(v["peer_order"]) or len(pp)!=len(peers) or set(pp)!=peers:raise ResearchContractError("experiment-design candidate or peer states do not partition")
        if not all(_digest(v.get(k)) for k in ("replay_identity","assurance_digest",a.get("content_hash"))) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=v.get("assurance_digest") or not all(_digest(x) for x in a.get("provenance_digests",[])):raise ResearchContractError("experiment-design assurance digest is invalid")
        if v["effect_receipts"]!=["block:unsafe-release"]:raise ResearchContractError("experiment-design assurance must remain verification-only")
def experiment_design_assurance_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"hubapi","consumers":["integration engineer","experiment-design steward","federated workflow operator"],"behavior":"verify federated continual experiment-design candidates and peer capability closure with deterministic evidence and policy witnesses","value":"prevents unsupported, underpowered, incomparable, or unauthorized designs from being mistaken for executable research plans","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact"],"permissions":["evaluate:capability-runs"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def assure_federated_experiment_design(request:Mapping[str,Any])->ExecutableExperimentDesign7:
    if request.get("schema_version")!=INPUT_SCHEMA or not all(str(request.get(k,"")).strip() for k in ("request_id","consumer","purpose","target_scope","semantic_profile")) or not request.get("required_modality_order") or not request.get("required_control_order") or int(request.get("required_peer_quorum",0))<=0 or int(request.get("checkpoint_seq",0))<=0 or not _digest(request.get("replay_identity")) or request.get("aggregate_only") is not True or request.get("raw_data_local") is not True or request.get("boundary")!=PRECLINICAL_BOUNDARY or not request.get("candidates") or not request.get("peers"):raise ResearchContractError("experiment objective identity, requirements, replay, locality, or boundary are invalid")
    candidates=sorted({str(x.get("design_id","")) for x in request["candidates"]});
    if len(candidates)!=len(request["candidates"]) or any(not x.strip() for x in candidates):raise ResearchContractError("design ids must be unique and non-empty")
    reqm=set(request["required_modality_order"]);reqc=set(request["required_control_order"]);q=set();u=set();b=set();mm=set();mc=set();om=set();unc=set();neg=set()
    for x in request["candidates"]:
        did=str(x["design_id"]);mods=set(x.get("modality_order",[]));ctrl=set(x.get("control_order",[]));missing_m=sorted(reqm-mods);missing_c=sorted(reqc-ctrl);mm.update(f"{did}:{m}" for m in missing_m);mc.update(f"{did}:{c}" for c in missing_c);neg.add(did) if x.get("negative_result") else None
        hard=not x.get("permitted") or not x.get("signed") or not x.get("local_only") or x.get("scope")!=request["target_scope"] or x.get("semantic_profile")!=request["semantic_profile"] or int(x.get("power_milli",0))<800 or not _digest(x.get("artifact_digest")) or not _digest(x.get("provenance_digest")) or x.get("replay_identity")!=request["replay_identity"] or not _ordered(x.get("modality_order",[])) or not _ordered(x.get("control_order",[]))
        if missing_m or missing_c:u.add(did);unc.add(f"{did}:required-closure")
        elif hard:b.add(did);om.add(f"{did}:design-integrity-or-policy")
        elif str(x.get("evidence_state")) in {"contradicted","unknown"}:u.add(did);unc.add(f"{did}:evidence-state")
        else:q.add(did)
        om.update(f"{did}:{o}" for o in x.get("omission_order",[]))
    peers=sorted({str(x.get("peer_id","")) for x in request["peers"]});
    if len(peers)!=len(request["peers"]) or any(not x.strip() for x in peers):raise ResearchContractError("peer ids must be unique and non-empty")
    qp=sorted({str(p["peer_id"]) for p in request["peers"] if p.get("signed") is True and p.get("policy_allowed") is True and p.get("local_only") is True and p.get("aggregate_only") is True and p.get("semantic_profile")==request["semantic_profile"] and p.get("capability_schema")==INPUT_SCHEMA and p.get("scope")==request["target_scope"] and int(p.get("checkpoint_seq",0))==int(request["checkpoint_seq"]) and _digest(p.get("attestation_digest"))});mp=sorted(set(peers)-set(qp));
    if mp:om.add(f"peer-quorum:{len(qp)}/{request['required_peer_quorum']}");unc.add("peer-closure-incomplete")
    for k,label in (("policy_allow","workflow:policy-denied"),("protected_closure","workflow:protected-closure-incomplete"),("federation_approved","workflow:federation-approval-missing"),("signed_approval","workflow:signed-approval-missing")):
        if request.get(k) is not True:om.add(label)
    global_block=any(request.get(k) is not True for k in ("policy_allow","protected_closure","federation_approved","signed_approval"));disp="blocked" if global_block or b else "partial" if not q or u or len(qp)<int(request["required_peer_quorum"]) else "qualified";
    if global_block:b.update(candidates);q.clear();u.clear()
    om.add("workflow:verification-only");checkpoint=_hash({"request_id":request["request_id"],"checkpoint_seq":int(request["checkpoint_seq"]),"target_scope":request["target_scope"],"replay_identity":request["replay_identity"]});payload={"candidate_order":candidates,"qualified_order":sorted(q),"unresolved_order":sorted(u),"blocked_order":sorted(b),"missing_modality_order":sorted(mm),"missing_control_order":sorted(mc),"peer_order":peers,"qualified_peer_order":qp,"missing_peer_order":mp,"omission_order":sorted(om),"uncertainty_order":sorted(unc),"negative_evidence_order":sorted(neg),"checkpoint_digest":checkpoint,"replay_identity":request["replay_identity"]};assurance=_hash(payload);payload.update({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"consumer":request["consumer"],"purpose":request["purpose"],"target_scope":request["target_scope"],"semantic_profile":request["semantic_profile"],"disposition":disp,"assurance_digest":assurance,"artifact":{"artifact_id":f"hubapi-experiment-design-assurance:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":assurance,"semantic_loss":["verification-only; no executable dispatch"],"provenance_digests":sorted({str(x.get("provenance_digest")) for x in request["candidates"]}),"boundary":PRECLINICAL_BOUNDARY},"effect_receipts":["block:unsafe-release"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY});r=ExecutableExperimentDesign7(payload);r.validate();return r
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","ExecutableExperimentDesign7","experiment_design_assurance_manifest","assure_federated_experiment_design"]
