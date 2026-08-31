"""Python parity for ``AFA-oraclex-P13-F20``.

This workbench qualifies declared analysis attestations; it never fits a model or exports raw
preclinical arrays.
"""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-oraclex-P13-F20"
CONTRACT_VERSION = "oraclex-federated-continual-statistical-causal-ml-analysis-research-workbench/1.0"
INPUT_SCHEMA = "AnalysisQuestion4@1"
OUTPUT_SCHEMA = "QualifiedAnalysisResult5@1"
CONTENT_TYPE = "application/vnd.aurora.oraclex-qualified-analysis-result-5+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _valid(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def statistical_analysis_research_workbench_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "oraclex", "consumers": ["preclinical neuroscientist", "analysis workbench operator", "federated research verifier"], "behavior": "qualify federated continual statistical, causal, and ML analysis attestations with identification, comparability, quality, evidence, provenance, replay, and locality gates without executing models", "value": "makes analytical readiness and uncertainty auditable before computation while retaining negative and omitted evidence", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["execute:local-computation", "write:local-artifact"], "permissions": ["analyze:declared-local-portfolio"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate_qualified_analysis_result(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified", "unresolved", "blocked"} or any(not isinstance(output.get(key), str) or not output[key].strip() for key in ("request_id", "researcher", "purpose", "semantic_profile")) or not output.get("candidate_order") or not output.get("study_order") or not output.get("modality_order") or not output.get("model_order") or not output.get("effect_receipts"):
        raise ResearchContractError("analysis identity, closure, locality, or effects are incomplete")
    keys = ("candidate_order", "selected_order", "unresolved_order", "blocked_order", "missing_candidate_order", "study_order", "selected_study_order", "unresolved_study_order", "blocked_study_order", "missing_study_order", "modality_order", "selected_modality_order", "unresolved_modality_order", "blocked_modality_order", "missing_modality_order", "model_order", "selected_model_order", "unresolved_model_order", "blocked_model_order", "missing_model_order", "omission_order", "uncertainty_order", "negative_evidence_order", "contradiction_order", "adversarial_event_order", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in keys):
        raise ResearchContractError("analysis ordering is not canonical")
    universe = sorted(set(output["candidate_order"]) | set(output["missing_candidate_order"]))
    for axis in (("candidate_order", "selected_order", "unresolved_order", "blocked_order", "missing_candidate_order"), ("study_order", "selected_study_order", "unresolved_study_order", "blocked_study_order", "missing_study_order"), ("modality_order", "selected_modality_order", "unresolved_modality_order", "blocked_modality_order", "missing_modality_order"), ("model_order", "selected_model_order", "unresolved_model_order", "blocked_model_order", "missing_model_order")):
        values = [item for key in axis[1:] for item in output.get(key, [])]
        if len(values) != len(set(values)) or set(values) != set(output[axis[0]] if axis[0] != "candidate_order" else universe):
            raise ResearchContractError("analysis states do not partition")
    if not _valid(output.get("replay_identity")) or not _valid(output.get("provenance_digest")) or not _valid(output.get("analysis_digest")) or artifact.get("content_hash") != output.get("analysis_digest"):
        raise ResearchContractError("analysis digest or artifact hash is inconsistent")
    effects = output["effect_receipts"]
    if any(effect != "block:unsafe-release" and not effect.startswith("analyze:local-portfolio:") for effect in effects):
        raise ResearchContractError("analysis effect is outside local portfolio gate")
    if output["disposition"] == "qualified" and effects != [f"analyze:local-portfolio:{output['request_id']}"]:
        raise ResearchContractError("qualified analysis effect is invalid")
    if output["disposition"] != "qualified" and effects != ["block:unsafe-release"]:
        raise ResearchContractError("non-qualified analysis must block")


def qualify_statistical_analysis(request: Mapping[str, Any]) -> dict[str, Any]:
    required = ("request_id", "researcher", "purpose", "semantic_profile")
    if request.get("schema_version") != INPUT_SCHEMA or any(not isinstance(request.get(key), str) or not request[key].strip() for key in required) or not all(request.get(key) for key in ("required_candidate_order", "required_study_order", "required_modality_order", "required_model_order", "candidates")) or any(not _ordered(request[key]) for key in ("required_candidate_order", "required_study_order", "required_modality_order", "required_model_order")) or not _ordered(request.get("adversarial_event_order", [])) or not _valid(request.get("replay_identity")) or request.get("budget_units", 0) <= 0 or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("analysis request identity, closure, replay, budget, locality, or boundary is invalid")
    rows = sorted((dict(row) for row in request["candidates"]), key=lambda row: (0 if row.get("evidence_state") == "proven" else 1 if row.get("evidence_state") == "supported" else 2 if row.get("evidence_state") in {"unknown", "speculative"} else 3, row.get("candidate_id", "")))
    ids = [row.get("candidate_id", "") for row in rows]
    if not all(ids) or len(ids) != len(set(ids)):
        raise ResearchContractError("analysis candidates must have unique identifiers")
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omission: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); contradiction: set[str] = set()
    for row in rows:
        cid = row["candidate_id"]; omission.update(f"{cid}:{item}" for item in row.get("omission_order", []))
        if row.get("negative_result"): negative.add(cid)
        if row.get("evidence_state") == "contradicted": contradiction.add(cid)
        hard = not all(row.get(key) is True for key in ("identification_supported", "comparability_supported", "quality_supported", "signed", "raw_data_local", "aggregate_only")) or row.get("evidence_state") == "contradicted"
        soft = row.get("replay_identity") != request["replay_identity"] or row.get("evidence_state") in {"unknown", "speculative"} or bool(row.get("omission_order"))
        if hard: blocked.add(cid)
        elif soft: unresolved.add(cid); uncertainty.add(f"{cid}:readiness-or-replay")
        else: selected.add(cid)
    required_ids = set(request["required_candidate_order"]); missing = required_ids - set(ids); omission.update(f"request:missing-candidate:{cid}" for cid in missing)
    axes = {"study": set(request["required_study_order"]), "modality": set(request["required_modality_order"]), "model": set(request["required_model_order"])}
    for row in rows: axes["study"].add(row.get("study_id", "")); axes["modality"].add(row.get("modality", "")); axes["model"].add(row.get("model_id", ""))
    def groups(axis: str) -> tuple[list[str], list[str], list[str], list[str]]:
        field = {"study": "study_id", "modality": "modality", "model": "model_id"}[axis]
        def has(target: str, group: set[str]) -> bool: return any(row.get(field) == target and row.get("candidate_id") in group for row in rows)
        a = sorted(item for item in axes[axis] if has(item, selected)); b = sorted(item for item in axes[axis] if item not in a and has(item, unresolved)); c = sorted(item for item in axes[axis] if item not in a and item not in b and has(item, blocked)); d = sorted(axes[axis] - set(a) - set(b) - set(c)); return a, b, c, d
    ss, us, bs, ms = groups("study"); sm, um, bm, mm = groups("modality"); sx, ux, bx, mx = groups("model")
    budget_used = len(rows); global_block = not all(request.get(key) is True for key in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order")) or budget_used > request["budget_units"]
    if global_block: blocked.update(ids); selected.clear(); unresolved.clear(); omission.add("control:policy-closure-approval-locality-or-budget-blocked")
    uncertainty.update(f"adversarial:{event}" for event in request.get("adversarial_event_order", []))
    selected_order, unresolved_order, blocked_order, missing_order = sorted(selected), sorted(unresolved), sorted(blocked), sorted(missing)
    disposition = "blocked" if global_block or blocked_order or missing_order or not selected_order else "unresolved" if unresolved_order or us or um or ux else "qualified"
    effects = [f"analyze:local-portfolio:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    provenance = _hash([row.get("provenance_digest") for row in rows]); value = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "researcher": request["researcher"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "candidate_order": ids, "selected_order": selected_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "missing_candidate_order": missing_order, "study_order": sorted(axes["study"]), "selected_study_order": ss, "unresolved_study_order": us, "blocked_study_order": bs, "missing_study_order": ms, "modality_order": sorted(axes["modality"]), "selected_modality_order": sm, "unresolved_modality_order": um, "blocked_modality_order": bm, "missing_modality_order": mm, "model_order": sorted(axes["model"]), "selected_model_order": sx, "unresolved_model_order": ux, "blocked_model_order": bx, "missing_model_order": mx, "omission_order": sorted(omission), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "contradiction_order": sorted(contradiction), "adversarial_event_order": sorted(request.get("adversarial_event_order", [])), "budget_used_units": budget_used, "replay_identity": request["replay_identity"], "provenance_digest": provenance, "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(value); value["analysis_digest"] = digest; value["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"oraclex-analysis-result-5:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; validate_qualified_analysis_result(value); return value


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "statistical_analysis_research_workbench_manifest", "qualify_statistical_analysis", "validate_qualified_analysis_result"]
