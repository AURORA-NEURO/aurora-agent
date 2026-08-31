"""Python parity for ``AFA-fiber-P05-F20`` federated resource workbench."""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-fiber-P05-F20"
CONTRACT_VERSION = "fiber-federated-continual-resource-discovery-research-workbench/1.0"
INPUT_SCHEMA = "ResourceNeed4@1"
OUTPUT_SCHEMA = "QualifiedResourceSet5@1"
CONTENT_TYPE = "application/vnd.aurora.fiber-qualified-resource-set-5+json"

def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: list[str]) -> bool: return values == sorted(set(values))
def _partition(value: Mapping[str, Any], universe: str, parts: tuple[str, ...], message: str) -> None:
    all_values = value.get(universe, []); classified = sum((value.get(part, []) for part in parts), [])
    if len(all_values) != len(set(all_values)) or len(classified) != len(set(classified)) or set(classified) != set(all_values): raise ResearchContractError(message)

@dataclass(frozen=True)
class FederatedResourceWorkbenchReceipt8:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        value = self.value; artifact = value.get("artifact", {})
        if not (value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and value.get("contract_version") == CONTRACT_VERSION and value.get("feature_id") == FEATURE_ID and value.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("boundary") == PRECLINICAL_BOUNDARY and value.get("raw_data_local") is True and value.get("aggregate_only") is True and all(isinstance(value.get(k), str) and value[k].strip() for k in ("request_id", "need_id", "requester", "intent", "semantic_profile")) and value.get("candidate_order") and value.get("ranked_order") and value.get("site_order") and value.get("effect_receipts") and value.get("disposition") in {"qualified", "unresolved", "blocked"}): raise ResearchContractError("resource identity, candidates, sites, locality, or effects are incomplete")
        for field in ("candidate_order", "selected_order", "unresolved_order", "blocked_order", "missing_candidate_order", "site_order", "selected_site_order", "missing_site_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts"):
            if not _ordered(value.get(field, [])): raise ResearchContractError("resource ordering is not canonical")
        _partition(value, "candidate_order", ("selected_order", "unresolved_order", "blocked_order", "missing_candidate_order"), "resource states do not form a complete partition")
        _partition(value, "site_order", ("selected_site_order", "missing_site_order"), "site states do not form a complete partition")
        if set(value["ranked_order"]) != set(value["candidate_order"]) or not all(_digest(value.get(k)) for k in ("resource_digest", "replay_identity")) or not _digest(artifact.get("content_hash")) or artifact.get("content_hash") != value.get("resource_digest") or artifact.get("content_type") != CONTENT_TYPE: raise ResearchContractError("resource ranking or digest is inconsistent")
        effects = value["effect_receipts"]
        if any(not effect.startswith("read:authorized-resource-state:") and effect != "block:unsafe-release" for effect in effects): raise ResearchContractError("effect is outside resource workbench gate")
        if value["disposition"] == "qualified" and effects != [f"read:authorized-resource-state:{value['need_id']}"]: raise ResearchContractError("qualified resource effect is invalid")
        if value["disposition"] != "qualified" and effects != ["block:unsafe-release"]: raise ResearchContractError("non-qualified resource workbench must block")
    def digest(self) -> str: self.validate(); return _hash(self.to_dict())

def federated_resource_workbench_manifest() -> dict[str, Any]: return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "fiber", "consumers": ["context compiler engineer", "preclinical researcher", "resource registry operator"], "behavior": "qualifies typed local and federated resource attestations against capability, site, evidence, trust, replay, and locality constraints with deterministic researcher-facing ranking", "value": "turns continual resource discovery into an auditable omission-aware workbench result", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["read_local_data", "write_local_artifact", "execute_local_computation"], "permissions": ["view:authorized-research-state"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

def _validate_request(request: Mapping[str, Any]) -> None:
    if not (request.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "need_id", "requester", "intent", "semantic_profile")) and request.get("required_capabilities") and request.get("required_site_order") and _ordered(request["required_site_order"]) and isinstance(request.get("minimum_site_count"), int) and request["minimum_site_count"] > 0 and isinstance(request.get("max_results"), int) and request["max_results"] > 0 and _digest(request.get("replay_identity")) and _ordered(request.get("adversarial_events", [])) and request.get("boundary") == PRECLINICAL_BOUNDARY and request.get("aggregate_only") is True and request.get("candidates")):
        raise ResearchContractError("resource request identity, capability closure, replay, boundary, or candidates are invalid")
    ids: set[str] = set()
    for candidate in request["candidates"]:
        if not (all(isinstance(candidate.get(k), str) and candidate[k].strip() for k in ("resource_id", "site_id", "semantic_profile")) and _digest(candidate.get("artifact_digest")) and _digest(candidate.get("provenance_digest")) and candidate["resource_id"] not in ids): raise ResearchContractError("resource candidate identity or digest is invalid")
        ids.add(candidate["resource_id"])

def qualify_federated_resources(request: Mapping[str, Any]) -> FederatedResourceWorkbenchReceipt8:
    _validate_request(request); candidates = sorted((dict(c) for c in request["candidates"]), key=lambda c: (-c.get("trust_score_milli", 0), c["resource_id"])); ranked_order = [c["resource_id"] for c in candidates]; candidate_order = sorted(ranked_order); selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for candidate in candidates:
        state = "selected"
        if not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "federation_allow", "signed_approval", "aggregate_only")) or candidate.get("revoked") or candidate.get("permitted") is not True or candidate.get("raw_data_local") is not True: state = "blocked"; omissions.add(f"resource:{candidate['resource_id']}:policy-or-locality")
        elif candidate.get("available") is not True or candidate["semantic_profile"] != request["semantic_profile"]: state = "unresolved"; uncertainty.add(f"resource:{candidate['resource_id']}:availability-or-semantic")
        elif not request["required_capabilities"] or not all(cap in candidate.get("capabilities", []) for cap in request["required_capabilities"]): state = "unresolved"; omissions.add(f"resource:{candidate['resource_id']}:capability-missing")
        elif candidate.get("evidence_state") in {"unknown", "speculative"}: state = "unresolved"; uncertainty.add(f"resource:{candidate['resource_id']}:evidence-not-asserted")
        elif candidate.get("evidence_state") == "contradicted": state = "blocked"; negative.add(f"resource:{candidate['resource_id']}:contradicted")
        if candidate.get("negative_result"): negative.add(f"resource:{candidate['resource_id']}:negative-result")
        if state == "selected":
            if len(selected) < request["max_results"]: selected.add(candidate["resource_id"])
            else: missing.add(candidate["resource_id"]); omissions.add(f"resource:{candidate['resource_id']}:result-limit")
        elif state == "unresolved": unresolved.add(candidate["resource_id"])
        else: blocked.add(candidate["resource_id"])
    sites = set(request["required_site_order"]) | {candidate["site_id"] for candidate in candidates}; selected_sites = {candidate["site_id"] for candidate in candidates if candidate["resource_id"] in selected}; missing_sites = {site for site in sites if site in request["required_site_order"] and site not in selected_sites}; omissions.update(f"site:{site}:missing-qualified-resource" for site in missing_sites); omissions.update(["control:policy-denied"] if request.get("policy_allow") is not True else []); omissions.update(["control:protected-closure-incomplete"] if request.get("protected_closure") is not True else []); omissions.update(["control:federation-denied"] if request.get("federation_allow") is not True else []); omissions.update(["control:signed-approval-missing"] if request.get("signed_approval") is not True else []); negative.update(f"adversarial:{event}" for event in request.get("adversarial_events", [])); global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "federation_allow", "signed_approval", "aggregate_only")) or bool(request.get("adversarial_events"))
    if global_block: blocked.update(candidate_order); selected.clear(); unresolved.clear(); missing.clear(); omissions.add("control:resource-release-gate-blocked")
    disposition = "blocked" if global_block or blocked else ("unresolved" if not selected or unresolved or missing or len(selected_sites) < request["minimum_site_count"] or missing_sites else "qualified"); omissions.add("control:resource-set-not-qualified") if disposition != "qualified" else None; effects = [f"read:authorized-resource-state:{request['need_id']}"] if disposition == "qualified" else ["block:unsafe-release"]; value = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "need_id": request["need_id"], "requester": request["requester"], "intent": request["intent"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "candidate_order": candidate_order, "ranked_order": ranked_order, "selected_order": sorted(selected), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "missing_candidate_order": sorted(missing), "site_order": sorted(sites), "selected_site_order": sorted(selected_sites), "missing_site_order": sorted(missing_sites), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"], "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}; resource_digest = _hash(value); value["resource_digest"] = resource_digest; value["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"qualified-resource-set-5:{request['need_id']}", "content_type": CONTENT_TYPE, "content_hash": resource_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; receipt = FederatedResourceWorkbenchReceipt8(value); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "FederatedResourceWorkbenchReceipt8", "federated_resource_workbench_manifest", "qualify_federated_resources"]
