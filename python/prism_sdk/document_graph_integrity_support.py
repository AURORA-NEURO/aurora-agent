"""Python parity for Docgraph P32 document-module lineage integrity cards."""
from __future__ import annotations
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
BOUNDARY = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions"
CONTENT_TYPE = "application/vnd.aurora.docgraph.document-module-integrity-card-1+json"
DocumentModule4 = dict[str, Any]; DocumentGraphIntegrityRequest4 = dict[str, Any]; DocumentGraphIntegrityCard7 = dict[str, Any]; DocumentGraphIntegrityArtifact4 = dict[str, Any]; DocumentGraphIntegrityError = ResearchContractError
def _hash(v: Any)->str: return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(",",":"),ensure_ascii=False).encode()).hexdigest()
def _digest(v: Any)->bool: return isinstance(v,str) and re.fullmatch(r"[0-9a-f]{64}",v) is not None
def _ordered(v:list[str])->bool: return isinstance(v,list) and v==sorted(set(v))
def manifest(*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]: return {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"docgraph","consumers":["documentation curator","context compiler","researcher workbench","release auditor"],"behavior":f"qualify typed document graph lineage at {scale} ({mode})","value":"prevents orphaned, cyclic, stale, or unauditable documentation context from entering research workflows","input_schema":"DocumentGraphIntegrityRequest4@1","output_schema":"DocumentGraphIntegrityCard7@1","effects":["emit:document-lineage-card","retain:rejected-and-unresolved-modules","block:unsafe-context-release"],"permissions":["read:local-document-manifests","exchange:aggregate-lineage"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY}
def _has_cycle(parent:Mapping[str,str])->bool:
    active:set[str]=set(); done:set[str]=set()
    def visit(node:str)->bool:
        if node in done:return False
        if node in active:return True
        active.add(node); nxt=parent.get(node)
        if nxt and nxt!="root" and visit(nxt):return True
        active.remove(node); done.add(node); return False
    return any(visit(node) for node in parent)
def validate(card:Mapping[str,Any],*,feature_id:str|None=None)->None:
    a=card.get("artifact",{})
    if card.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or (feature_id is not None and card.get("feature_id")!=feature_id) or not card.get("request_id") or not card.get("purpose") or card.get("boundary")!=BOUNDARY or a.get("boundary")!=BOUNDARY or card.get("raw_data_local") is not True or card.get("aggregate_only") is not True or not _digest(card.get("replay_identity")) or not _digest(card.get("closure_digest")) or a.get("content_type")!=CONTENT_TYPE or a.get("content_hash")!=card.get("closure_digest") or card.get("admitted_module_count",0)>card.get("total_module_count",0): raise ResearchContractError("document identity, locality, artifact, digest, boundary, or count is incomplete")
    for key in ("module_order","admitted_order","rejected_order","unknown_order","omitted_order","lineage_order","consumer_order","contract_order","effect_receipts"):
        if not _ordered(card.get(key,[])): raise ResearchContractError("document graph vectors are not canonical")
    ids=set(card["module_order"]); states=set(card["admitted_order"])|set(card["rejected_order"])|set(card["unknown_order"])|set(card["omitted_order"])
    if len(card["module_order"])!=len(ids) or states!=ids: raise ResearchContractError("document module states do not partition modules")
    if card["admitted_module_count"]!=len(card["admitted_order"]): raise ResearchContractError("admitted module count does not match admitted order")
def qualify(request:Mapping[str,Any],*,feature_id:str,contract_version:str,scale:str,mode:str)->dict[str,Any]:
    if request.get("schema_version")!=RESEARCH_CONTRACT_SCHEMA_VERSION or not request.get("request_id","").strip() or not request.get("purpose","").strip() or not request.get("modules") or request.get("module_budget",0)<=0 or not _digest(request.get("replay_identity")) or request.get("boundary")!=BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or not _ordered(request.get("required_module_order",[])) or not _ordered(request.get("adversarial_events",[])): raise ResearchContractError("document identity, ordering, replay, locality, boundary, or budget is invalid")
    rows=sorted(request["modules"],key=lambda item:item.get("module_id","")); seen:set[str]=set(); parent:dict[str,str]={}; admitted:set[str]=set(); rejected:set[str]=set(); unknown:set[str]=set(); omitted:set[str]=set(); lineage:set[str]=set(); consumers:set[str]=set(); contracts:set[str]=set(); effects:set[str]=set(); sources:set[str]=set(); semantic_loss:list[str]=[]
    for m in rows:
        mid=m.get("module_id","")
        if not mid.strip() or not m.get("parent_module","").strip() or not m.get("owner_crate","").strip() or not m.get("consumer","").strip() or not m.get("behavior","").strip() or not m.get("input_schema","").strip() or not m.get("output_schema","").strip() or not _digest(m.get("source_digest")) or not m.get("evidence_state","").strip() or m.get("local") is not True or m.get("aggregate_only") is not True: raise ResearchContractError("module identity, lineage, consumer, typed ports, evidence, or locality is incomplete")
        if mid in seen: raise ResearchContractError(f"duplicate document module {mid}")
        seen.add(mid); parent[mid]=m["parent_module"]; lineage.add(f"{mid}<-{m['parent_module']}"); consumers.add(m["consumer"]); contracts.add(f"{m['input_schema']}→{m['output_schema']}"); effects.add(f"document:{mid}"); sources.add(m["source_digest"]); state=m["evidence_state"]
        if state in ("supported","proven") and m.get("required") is True and m.get("deterministic") is True: admitted.add(mid)
        elif state in ("contradicted","rejected"): rejected.add(mid); semantic_loss.append(mid)
        elif state in ("unknown","speculative","unmeasured"): unknown.add(mid); semantic_loss.append(mid)
        else: omitted.add(mid); semantic_loss.append(mid)
    if any(ancestor!="root" and ancestor not in seen for ancestor in parent.values()) or _has_cycle(parent): raise ResearchContractError("document graph has an orphan parent or cycle")
    if set(request["required_module_order"])!=seen: raise ResearchContractError("required module order is not the canonical module set")
    global_block=request.get("policy_allowed") is not True or request.get("protected_closure") is not True or request.get("signed_manifest") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events")) or len(rows)>request["module_budget"]
    if global_block: omitted.update(seen); admitted.clear(); rejected.clear(); unknown.clear()
    disposition="blocked" if global_block else "unknown" if unknown else "partial" if rejected or omitted else "qualified"; body={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request["request_id"],"purpose":request["purpose"],"disposition":disposition,"module_order":sorted(seen)}; closure_digest=_hash(body); admitted_order=sorted(admitted); rejected_order=sorted(rejected); unknown_order=sorted(unknown); omitted_order=sorted(omitted)
    card={**body,"admitted_order":admitted_order,"rejected_order":rejected_order,"unknown_order":unknown_order,"omitted_order":omitted_order,"lineage_order":sorted(lineage),"consumer_order":sorted(consumers),"contract_order":sorted(contracts),"replay_identity":request["replay_identity"],"closure_digest":closure_digest,"admitted_module_count":len(admitted_order),"total_module_count":len(rows),"raw_data_local":True,"aggregate_only":True,"boundary":BOUNDARY,"effect_receipts":[f"prepare:document-lineage:{request['request_id']}"] if disposition=="qualified" else ["block:unsafe-context-release"],"artifact":{"artifact_id":f"docgraph-lineage:{request['request_id']}","content_type":CONTENT_TYPE,"content_hash":closure_digest,"semantic_loss":sorted(seen) if global_block else semantic_loss,"source_digests":sorted(sources),"boundary":BOUNDARY}}
    validate(card,feature_id=feature_id); return card
__all__=["BOUNDARY","CONTENT_TYPE","DocumentModule4","DocumentGraphIntegrityRequest4","DocumentGraphIntegrityCard7","DocumentGraphIntegrityArtifact4","DocumentGraphIntegrityError","manifest","qualify","validate"]
