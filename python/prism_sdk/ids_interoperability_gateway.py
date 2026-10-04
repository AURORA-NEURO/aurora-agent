"""Python parity for ``AFA-ids-P22-F24`` interoperability negotiation."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-ids-P22-F24"; CONTRACT_VERSION="ids-version-negotiated-interoperability-extensibility-gateway/1.0"; INPUT_SCHEMA="ExternalCapability8@1"; OUTPUT_SCHEMA="NegotiatedIntegration9@1"; CONTENT_TYPE="application/vnd.aurora.negotiated-integration-9+json"
def _hash(v:Any)->str:return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v:Any)->bool:return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool:return v==sorted(set(v))
@dataclass(frozen=True)
class NegotiatedIntegration9:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value; a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(v.get(k," ").strip() for k in ("request_id","purpose","semantic_profile","required_capability")) or not v.get("endpoint_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}: raise ResearchContractError("interoperability identity, locality, endpoints, disposition, or effects are incomplete")
        fields=("endpoint_order","accepted_order","migrated_order","incompatible_order","unresolved_order","blocked_order","missing_capability_order","omission_order","uncertainty_order","negative_evidence_order","migration_loss","effect_receipts")
        if any(not _ordered(v.get(k,[])) for k in fields):raise ResearchContractError("interoperability ordering is not canonical")
        ids=set(v["endpoint_order"]); parts=v["accepted_order"]+v["migrated_order"]+v["incompatible_order"]+v["unresolved_order"]+v["blocked_order"]
        if len(ids)!=len(v["endpoint_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("endpoint states do not partition")
        if not all(_digest(d) for d in [v.get("replay_identity"),v.get("integration_digest"),a.get("content_hash"),*a.get("provenance_digests",[])]) or a.get("content_hash")!=v.get("integration_digest") or a.get("content_type")!=CONTENT_TYPE:raise ResearchContractError("integration digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:integration-manifests:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("interoperability effect is outside governed gate")
def interoperability_gateway_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["protocol adapter","SDK integrator","federation operator","compatibility auditor"],"behavior":"negotiate versioned external capability manifests with explicit migration loss and conformance gates","value":"prevents semantic drift, incompatible versions, unauthorized effects, and raw-data movement at extension boundaries","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:integration-manifests","manage:local-capability"],"permissions":["read:capability-manifests","request:version-negotiation"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","purpose","semantic_profile","required_capability")) or not r.get("supported_versions") or not r.get("capabilities") or len(r["capabilities"])>8192 or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("interoperability identity, versions, capability bound, replay, or locality is invalid")
    ids:set[str]=set()
    for c in r["capabilities"]:
        if not all(isinstance(c.get(k),str) and c[k].strip() for k in ("capability_id","endpoint_id","semantic_profile")) or not c.get("offered_versions") or not _digest(c.get("input_digest")) or not _digest(c.get("provenance_digest")) or not _digest(c.get("replay_identity")) or not c.get("effects") or c["endpoint_id"] in ids:raise ResearchContractError("capability identity, versions, semantic profile, digests, effects, or uniqueness is invalid")
        ids.add(c["endpoint_id"])
def negotiate_interoperability(r:Mapping[str,Any])->NegotiatedIntegration9:
    _validate_request(r); caps=sorted((dict(c) for c in r["capabilities"]),key=lambda c:c["endpoint_id"]); endpoints=[c["endpoint_id"] for c in caps]; supported=set(r["supported_versions"]); accepted:set[str]=set(); migrated:set[str]=set(); incompatible:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); losses:set[str]=set(); selected=""
    for c in caps:
        i=c["endpoint_id"]
        if c["capability_id"]!=r["required_capability"]:incompatible.add(i);omissions.add(f"{i}:capability-mismatch")
        elif c.get("local") is not True or c.get("aggregate_only") is not True:blocked.add(i);omissions.add(f"{i}:raw-data-locality")
        elif c["replay_identity"]!=r["replay_identity"]:unresolved.add(i);uncertainty.add(f"{i}:replay-identity")
        elif c["semantic_profile"]!=r["semantic_profile"]:incompatible.add(i);omissions.add(f"{i}:semantic-profile")
        elif c.get("evidence_state")=="contradicted":blocked.add(i);negative.add(f"{i}:contradicted")
        elif c.get("evidence_state") not in {"proven","supported"}:unresolved.add(i);uncertainty.add(f"{i}:evidence-state")
        else:
            common=sorted(set(c["offered_versions"])&supported)
            if not common:incompatible.add(i);omissions.add(f"{i}:version-incompatible")
            else:
                ver=common[-1];accepted.add(i);selected=max(selected,ver);losses.update(c.get("migration_loss",[]))
    missing=[] if accepted or migrated else [r["required_capability"]]
    if missing:omissions.add(f"capability:{r['required_capability']}:missing")
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","federation_approved","raw_data_local","aggregate_only"))
    if global_block:blocked.update(endpoints);accepted.clear();migrated.clear();unresolved.clear();incompatible.clear();omissions.add("request:governance-or-locality-denied")
    ao,mo,io,uo,bo=(sorted(x) for x in (accepted,migrated,incompatible,unresolved,blocked)); disp="blocked" if global_block or (not ao and not mo and not uo) else "unresolved" if uo or bo or io or missing else "qualified"
    if disp!="qualified":omissions.add("request:integration-not-closed")
    p={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"purpose":r["purpose"],"semantic_profile":r["semantic_profile"],"required_capability":r["required_capability"],"disposition":disp,"endpoint_order":endpoints,"accepted_order":ao,"migrated_order":mo,"incompatible_order":io,"unresolved_order":uo,"blocked_order":bo,"missing_capability_order":missing,"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"negotiated_version":selected,"migration_loss":sorted(losses),"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(p);p["integration_digest"]=digest;p["artifact"]={"artifact_id":f"negotiated-integration-9:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":sorted(omissions),"provenance_digests":sorted({c["provenance_digest"] for c in caps}),"boundary":PRECLINICAL_BOUNDARY};p["effect_receipts"]=sorted([f"exchange:integration-manifests:{r['request_id']}",f"manage:local-capability:{r['request_id']}"] if disp=="qualified" else ["block:unsafe-release"]); out=NegotiatedIntegration9(p);out.validate();return out
def idsInteroperabilityGatewayDigest(result:NegotiatedIntegration9)->str:result.validate();return _hash(result.to_dict())
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","NegotiatedIntegration9","interoperability_gateway_manifest","negotiate_interoperability","idsInteroperabilityGatewayDigest"]
