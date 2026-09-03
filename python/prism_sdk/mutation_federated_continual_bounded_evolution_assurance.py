"""Python parity for ``AFA-mutation-P32-F28`` federated continual bounded-evolution assurance."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID="AFA-mutation-P32-F28"; CONTRACT_VERSION="mutation-federated-continual-bounded-evolution-assurance/1.0"; INPUT_SCHEMA="MutationEvolutionRequest8@1"; OUTPUT_SCHEMA="MutationEvolutionReceipt10@1"; CONTENT_TYPE="application/vnd.aurora.mutation-federated-evolution-decision+json"; MAX_PROPOSALS=16_384
def _hash(value:Any)->str:return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(value:Any)->bool:return isinstance(value,str) and re.fullmatch(r"[0-9a-f]{64}",value) is not None
def _ordered(values:list[str])->bool:return values==sorted(set(values))
@dataclass(frozen=True)
class MutationEvolutionReceipt10:
    value:dict[str,Any]
    def to_dict(self)->dict[str,Any]:return dict(self.value)
    def validate(self)->None:
        v=self.value; a=v.get("artifact",{})
        if v.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version")!=CONTRACT_VERSION or v.get("feature_id")!=FEATURE_ID or v.get("boundary")!=PRECLINICAL_BOUNDARY or a.get("boundary")!=PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not all(isinstance(v.get(k),str) and v[k].strip() for k in ("request_id","purpose","capability_id","current_version")) or not v.get("proposal_order") or not v.get("effect_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified","unresolved","blocked"}:raise ResearchContractError("mutation evolution identity, proposals, effects, locality, or disposition is incomplete")
        for k in ("proposal_order","approved_order","unresolved_order","blocked_order","incompatible_order","benchmark_failed_order","safety_failed_order","omission_order","uncertainty_order","negative_evidence_order","effect_order","effect_receipts"):
            if not _ordered(v.get(k,[])):raise ResearchContractError("evolution ordering is not canonical")
        ids=set(v["proposal_order"]); parts=v["approved_order"]+v["unresolved_order"]+v["blocked_order"]
        if len(ids)!=len(v["proposal_order"]) or len(parts)!=len(ids) or set(parts)!=ids:raise ResearchContractError("evolution states do not partition")
        if not _digest(v.get("replay_identity")) or not _digest(v.get("evolution_digest")) or a.get("content_hash")!=v.get("evolution_digest") or a.get("content_type")!=CONTENT_TYPE or any(not _digest(d) for d in a.get("provenance_digests",[])):raise ResearchContractError("evolution digest or artifact metadata is inconsistent")
        if any(not e.startswith(("preview:bounded-evolution:","manage:local-capability:")) and e!="block:unsafe-release" for e in v["effect_receipts"]):raise ResearchContractError("effect is outside the governed mutation evolution gate")
    def digest(self)->str:self.validate();return _hash(self.to_dict())
def mutation_federated_bounded_evolution_manifest()->dict[str,Any]:return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"mutation","consumers":["researcher","release operator","evolution steward"],"behavior":"evaluate signed capability-evolution proposals with deterministic compatibility, benchmark, safety, evidence, replay, policy, locality, and protected-closure gates","value":"makes bounded evolution auditable and reversible before any implementation or release mutation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["preview:bounded-evolution","manage:local-capability"],"permissions":["read:local-evolution-summaries","request:bounded-evolution-preview"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY}
def _validate_request(r:Mapping[str,Any])->None:
    if not all(isinstance(r.get(k),str) and r[k].strip() for k in ("request_id","purpose","capability_id","current_version")) or not r.get("proposals") or len(r["proposals"])>MAX_PROPOSALS or not _digest(r.get("replay_identity")) or r.get("boundary")!=PRECLINICAL_BOUNDARY or r.get("raw_data_local") is not True or r.get("aggregate_only") is not True:raise ResearchContractError("mutation evolution identity, proposal bound, replay, locality, or boundary is invalid")
    ids:set[str]=set()
    for p in r["proposals"]:
        if not isinstance(p,Mapping) or not isinstance(p.get("proposal_id"),str) or not p["proposal_id"].strip() or p["proposal_id"] in ids or not isinstance(p.get("capability_id"),str) or not p["capability_id"].strip() or not isinstance(p.get("from_version"),str) or not p["from_version"].strip() or not isinstance(p.get("to_version"),str) or not p["to_version"].strip() or not _digest(p.get("artifact_digest")) or not _digest(p.get("benchmark_digest")) or not _digest(p.get("replay_identity")) or p.get("local") is not True or p.get("aggregate_only") is not True or p.get("evidence_state") not in {"proven","supported","unknown","unmeasured","contradicted"}:raise ResearchContractError(f"proposal {p.get('proposal_id','')} is invalid, duplicated, non-local, or not digest-bound")
        ids.add(p["proposal_id"])
def assure_mutation_federated_bounded_evolution(r:Mapping[str,Any])->MutationEvolutionReceipt10:
    _validate_request(r); ps=sorted((dict(p) for p in r["proposals"]),key=lambda p:p["proposal_id"]); order=[p["proposal_id"] for p in ps]; approved:set[str]=set(); unresolved:set[str]=set(); blocked:set[str]=set(); incompatible:set[str]=set(); benchmark:set[str]=set(); safety:set[str]=set(); omissions:set[str]=set(); uncertainty:set[str]=set(); negative:set[str]=set(); provenance:set[str]=set()
    for p in ps:
        provenance.update((p["artifact_digest"],p["benchmark_digest"])); pid=p["proposal_id"]
        if p["capability_id"]!=r["capability_id"] or p["from_version"]!=r["current_version"] or not p["compatible"]:unresolved.add(pid);incompatible.add(pid)
        elif not p["benchmark_pass"]:unresolved.add(pid);benchmark.add(pid);negative.add(f"{pid}:benchmark-failed")
        elif not p["safety_pass"]:blocked.add(pid);safety.add(pid);negative.add(f"{pid}:safety-failed")
        elif p["replay_identity"]!=r["replay_identity"]:unresolved.add(pid);uncertainty.add(f"{pid}:replay-identity")
        elif p["signed"] is not True:blocked.add(pid);omissions.add(f"{pid}:unsigned")
        elif p["evidence_state"]=="contradicted":blocked.add(pid);negative.add(f"{pid}:contradicted")
        elif p["evidence_state"] not in {"proven","supported"}:unresolved.add(pid);uncertainty.add(f"{pid}:evidence-state")
        else:approved.add(pid)
    global_block=not all(r.get(k) is True for k in ("policy_allow","protected_closure","signed_approval","raw_data_local","aggregate_only"))
    if global_block:blocked.update(order);approved.clear();unresolved.clear();omissions.add("request:governance-or-locality-denied")
    ao=sorted(approved);uo=sorted(unresolved);bo=sorted(blocked);disp="blocked" if global_block or (not ao and not uo) else ("unresolved" if bo or uo else "qualified")
    if disp!="qualified":omissions.add("request:bounded-evolution-not-closed")
    effects=sorted(["manage:local-capability","preview:bounded-evolution"] if disp=="qualified" else ["block:unsafe-release"])
    payload={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r["request_id"],"purpose":r["purpose"],"capability_id":r["capability_id"],"current_version":r["current_version"],"disposition":disp,"proposal_order":order,"approved_order":ao,"unresolved_order":uo,"blocked_order":bo,"incompatible_order":sorted(incompatible),"benchmark_failed_order":sorted(benchmark),"safety_failed_order":sorted(safety),"omission_order":sorted(omissions),"uncertainty_order":sorted(uncertainty),"negative_evidence_order":sorted(negative),"effect_order":effects,"replay_identity":r["replay_identity"],"raw_data_local":True,"aggregate_only":True,"boundary":PRECLINICAL_BOUNDARY}; digest=_hash(payload);v=dict(payload);v["evolution_digest"]=digest;v["artifact"]={"artifact_id":f"mutation-federated-evolution-decision-7:{r['request_id']}","content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":sorted(provenance),"boundary":PRECLINICAL_BOUNDARY};v["effect_receipts"]=sorted(e if e=="block:unsafe-release" else f"{e}:{r['request_id']}" for e in effects);receipt=MutationEvolutionReceipt10(v);receipt.validate();return receipt
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","CONTENT_TYPE","MutationEvolutionReceipt10","mutation_federated_bounded_evolution_manifest","assure_mutation_federated_bounded_evolution"]
