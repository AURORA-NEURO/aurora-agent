"""Parity surface for ``AFA-stress-P16-F20`` publication workbench."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-stress-P16-F20"
CONTRACT_VERSION = "stress-federated-continual-publication-research-object-workbench/1.0"
INPUT_SCHEMA = "ValidatedResearchRun4@1"
OUTPUT_SCHEMA = "SignedResearchObject5@1"
CONTENT_TYPE = "application/vnd.aurora.signed-research-object-5+json"

def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))

def publication_research_object_workbench_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "stress", "consumers": ["benchmark curator", "research-object publisher", "release reviewer"], "behavior": "compile digest-only validated research runs into an omission-aware portable research-object release envelope without signing or publishing", "value": "preserves replay, provenance, negative results, and unknown versus unmeasured evidence", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["read:local-data", "write:local-artifact"], "permissions": ["view:authorized-research-state"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

def validate_publication_research_object(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("disposition") not in {"qualified", "conditional", "blocked", "unknown"} or not output.get("run_order") or not output.get("effect_receipts"):
        raise ResearchContractError("research-object identity, locality, disposition, runs, or effects are incomplete")
    keys = ("run_order", "qualified_order", "conditional_order", "blocked_order", "unknown_order", "omission_order", "uncertainty_order", "negative_evidence_order", "required_standards", "covered_standards", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in keys):
        raise ResearchContractError("research-object ordering is not canonical")
    ids = set(output["run_order"]); states = set(output["qualified_order"] + output["conditional_order"] + output["blocked_order"] + output["unknown_order"])
    if len(ids) != len(output["run_order"]) or states != ids:
        raise ResearchContractError("run dispositions do not partition")
    if not all(_valid_digest(output.get(key)) for key in ("signature_digest", "replay_identity", "research_object_digest")) or artifact.get("content_hash") != output.get("research_object_digest"):
        raise ResearchContractError("research-object digest or signature metadata is invalid")

def compile_publication_research_object(request: Mapping[str, Any]) -> dict[str, Any]:
    objective = request.get("objective", {})
    if request.get("schema_version") != INPUT_SCHEMA or not all(isinstance(objective.get(key), str) and objective[key].strip() for key in ("semantic_profile",)) or not request.get("required_standards") or not _ordered(request["required_standards"]) or not request.get("runs") or not _valid_digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("request identity, standards, runs, replay, or boundary is invalid")
    runs = sorted((dict(run) for run in request["runs"]), key=lambda run: run.get("run_id", ""))
    ids = [run["run_id"] for run in runs]
    if len(set(ids)) != len(ids):
        raise ResearchContractError("run ids must be unique")
    qualified: set[str] = set(); conditional: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); covered: set[str] = set()
    for run in runs:
        rid = run["run_id"]; covered.update(run.get("standards", [])); omissions.update(f"{rid}:{item}" for item in run.get("omission_order", []))
        if run.get("negative_result"): negative.add(f"{rid}:negative-result")
        missing = sorted(set(request["required_standards"]) - set(run.get("standards", [])))
        hard = not run.get("policy_allow") or not run.get("protected_closure") or not run.get("raw_data_local") or run.get("semantic_profile") != request["semantic_profile"] or not all(_valid_digest(run.get(key)) for key in ("artifact_digest", "evidence_digest", "provenance_digest"))
        if hard:
            blocked.add(rid); omissions.add(f"{rid}:policy-provenance-locality-or-semantic-blocked")
        elif missing or run.get("reproducibility_score", 0) < 80 or run.get("evidence_state") in {"unknown", "speculative"}:
            conditional.add(rid); uncertainty.add(f"{rid}:release-closure-incomplete"); omissions.update(f"{rid}:missing-standard:{standard}" for standard in missing)
        elif run.get("evidence_state") == "contradicted":
            unknown.add(rid); negative.add(f"{rid}:contradicted-evidence")
        else: qualified.add(rid)
    global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "raw_data_local"))
    if global_block:
        blocked.update(ids); qualified.clear(); conditional.clear(); unknown.clear(); omissions.add("request:policy-protected-closure-or-locality-blocked")
    disposition = "blocked" if global_block or (blocked and not qualified) else "conditional" if conditional or unknown or blocked else "qualified"
    if disposition != "qualified": omissions.add("request:release-closure-not-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "consumer": request["consumer"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "run_order": ids, "qualified_order": sorted(qualified), "conditional_order": sorted(conditional), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "required_standards": sorted(request["required_standards"]), "covered_standards": sorted(covered), "replay_identity": request["replay_identity"], "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    research_digest = _digest(payload)
    payload["research_object_digest"] = research_digest
    payload["signature_digest"] = _digest({"research_object_digest": research_digest, "replay_identity": request["replay_identity"], "contract_version": CONTRACT_VERSION})
    payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"stress-research-object:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": research_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}
    payload["effect_receipts"] = [f"view:authorized-research-state:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate_publication_research_object(payload)
    return payload

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "publication_research_object_workbench_manifest", "compile_publication_research_object", "validate_publication_research_object"]
