"""Approval-bounded knowledge graph copilot for Worldgen P04 F09-F12."""
from __future__ import annotations
from dataclasses import dataclass
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest
from .worldgen_knowledge_representation_support import KnowledgeRepresentationRequest, represent

CONTENT_TYPE = "application/vnd.aurora.worldgen.knowledge-copilot-receipt+json"

@dataclass(frozen=True)
class KnowledgeCopilotRequest:
    knowledge_request: KnowledgeRepresentationRequest
    action_order: tuple[str, ...]
    action_budget: int
    dry_run: bool = False
    signed_approval: bool = False
    federation_approved: bool = False
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class KnowledgeCopilotReceipt:
    value: dict[str, Any]
    def validate(self) -> None:
        v, artifact = self.value, self.value.get("artifact", {})
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("boundary") != PRECLINICAL_BOUNDARY or
            artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or
            not v.get("raw_data_local") or not v.get("aggregate_only") or not v.get("request_id") or not v.get("action_order") or not v.get("effect_receipts")):
            raise ResearchContractError("knowledge copilot identity, locality, actions, or effects are incomplete")
        for key in ("action_order", "admitted_action_order", "denied_action_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            vals = tuple(v.get(key, ()))
            if vals != tuple(sorted(set(vals))):
                raise ResearchContractError("knowledge copilot ordering is not canonical")
        actions, admitted, denied = set(v["action_order"]), set(v.get("admitted_action_order", ())), set(v.get("denied_action_order", ()))
        if len(actions) != len(v["action_order"]) or admitted | denied != actions or len(admitted) + len(denied) != len(actions):
            raise ResearchContractError("knowledge copilot actions do not partition")
        for key in ("graph_digest", "copilot_digest", "replay_identity"):
            value = v.get(key, "")
            if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
                raise ResearchContractError("knowledge copilot digest is invalid")
        if artifact.get("content_hash") != v.get("copilot_digest"):
            raise ResearchContractError("knowledge copilot artifact digest is inconsistent")
        if any(e != "block:unsafe-release" and not e.startswith("invoke:bounded-knowledge-tool:") for e in v["effect_receipts"]):
            raise ResearchContractError("knowledge copilot effect is outside bounded-tool gate")
    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen",
            "consumers": ["knowledge engineer", "preclinical researcher", "graph compiler"], "behavior": f"run bounded typed knowledge-graph actions for {scale}",
            "value": "turns graph representation into an approval-bounded agent product without hiding unsupported edges", "input_schema": input_schema,
            "output_schema": "KnowledgeCopilotReceipt1@1", "effects": ["invoke:bounded-knowledge-tool", "block:unsafe-release"],
            "permissions": ["invoke:declared-knowledge-tool"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY, "contract_version": contract_version}

def run(request: KnowledgeCopilotRequest, *, feature_id: str, contract_version: str, scale: str, require_approval: bool = False, require_federation: bool = False) -> KnowledgeCopilotReceipt:
    if request.boundary != PRECLINICAL_BOUNDARY or request.action_budget <= 0 or not request.action_order or tuple(request.action_order) != tuple(sorted(set(request.action_order))) or not request.knowledge_request.raw_data_local or not request.knowledge_request.aggregate_only:
        raise ResearchContractError("knowledge copilot request is invalid")
    graph = represent(request.knowledge_request, feature_id=feature_id, contract_version=contract_version, scale=scale, require_federation=require_federation)
    omissions = list(graph.value["omissions"])
    approved = (not require_approval or request.signed_approval) and (not require_federation or request.federation_approved)
    if not approved: omissions.append("copilot:approval-missing")
    if request.dry_run: omissions.append("copilot:dry-run-no-effect")
    if len(request.action_order) > request.action_budget: omissions.append("copilot:action-budget-exceeded")
    safe = graph.value["disposition"] == "qualified" and approved and len(request.action_order) <= request.action_budget
    disposition = "qualified" if safe else "blocked" if graph.value["disposition"] == "blocked" or not approved else "partial"
    admitted, denied = (list(request.action_order), []) if safe else ([], list(request.action_order))
    effects = [f"invoke:bounded-knowledge-tool:{request.knowledge_request.request_id}"] if disposition != "blocked" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id,
               "request_id": request.knowledge_request.request_id, "disposition": disposition, "action_order": sorted(request.action_order),
               "admitted_action_order": sorted(admitted), "denied_action_order": sorted(denied), "knowledge_disposition": graph.value["disposition"],
               "graph_digest": graph.value["graph_digest"], "replay_identity": graph.value["replay_identity"], "copilot_digest": "",
               "omissions": sorted(set(omissions)), "uncertainty": graph.value["uncertainty"], "negative_evidence": graph.value["negative_evidence"],
               "effect_receipts": sorted(effects), "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest_payload = dict(payload)
    digest_payload.pop("copilot_digest", None)
    digest = research_artifact_digest(digest_payload)
    payload["copilot_digest"] = digest
    payload["artifact"] = {"artifact_id": f"knowledge-copilot:{request.knowledge_request.request_id}", "content_type": CONTENT_TYPE, "content_hash": digest, "boundary": PRECLINICAL_BOUNDARY}
    receipt = KnowledgeCopilotReceipt(payload)
    receipt.validate()
    return receipt

__all__ = ["CONTENT_TYPE", "KnowledgeCopilotRequest", "KnowledgeCopilotReceipt", "manifest", "run"]
