"""Parity surface for ``AFA-epistemic-P09-F19``."""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-epistemic-P09-F19"
CONTRACT_VERSION = "epistemic-prospective-high-throughput-experiment-design-research-workbench/1.0"
INPUT_SCHEMA = "ExperimentObjective3@1"
OUTPUT_SCHEMA = "ExecutableExperimentDesign5@1"
CONTENT_TYPE = "application/vnd.aurora.executable-experiment-design-5+json"


def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


def experiment_design_research_workbench_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "epistemic", "consumers": ["downstream AURORA crate maintainer", "preclinical design scientist", "research workbench operator"], "behavior": "rank typed power-aware experiment designs into a deterministic prospective workbench contract with explicit omissions, uncertainty, and evidence gates", "value": "makes high-throughput design trade-offs and power shortfalls auditable before any laboratory action", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["execute:local-computation", "write:local-artifact"], "permissions": ["view:authorized-research-state"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}


def validate_executable_experiment_design(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("disposition") not in {"qualified", "partial", "blocked"} or not output.get("candidate_order") or not output.get("plan_order") or not output.get("evidence_order") or not output.get("effect_receipts"):
        raise ResearchContractError("workbench identity, plan, evidence, locality, or effects are incomplete")
    keys = ("candidate_order", "executable_order", "unresolved_order", "blocked_order", "missing_candidate_order", "missing_factor_order", "plan_order", "evidence_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
    if any(not _ordered(output.get(key, [])) for key in keys):
        raise ResearchContractError("workbench ordering is not canonical")
    ids = set(output["candidate_order"]); partitions = output["executable_order"] + output["unresolved_order"] + output["blocked_order"]
    if len(ids) != len(output["candidate_order"]) or len(partitions) != len(set(partitions)) or set(partitions) != ids or set(output["ranked_order"]) != ids:
        raise ResearchContractError("candidate states do not partition")
    if not all(_valid_digest(output.get(key)) for key in ("baseline_digest", "replay_identity", "plan_digest")) or artifact.get("content_hash") != output.get("plan_digest"):
        raise ResearchContractError("workbench digest or artifact hash is inconsistent")
    if output["disposition"] == "qualified":
        if len(output["effect_receipts"]) != 1 or not output["effect_receipts"][0].startswith("view:design-plan:"):
            raise ResearchContractError("qualified workbench effect is invalid")
    elif output["effect_receipts"] != ["block:unsafe-release"]:
        raise ResearchContractError("non-qualified workbench must block release")


def compile_experiment_design_workbench(objective: Mapping[str, Any], candidates: list[Mapping[str, Any]]) -> dict[str, Any]:
    if objective.get("schema_version") != INPUT_SCHEMA or not all(isinstance(objective.get(key), str) and objective[key].strip() for key in ("request_id", "researcher_id", "study_program", "purpose", "scope", "semantic_profile")) or not objective.get("required_candidate_order") or not _ordered(objective["required_candidate_order"]) or not _ordered(objective.get("required_factor_order", [])) or not _valid_digest(objective.get("baseline_digest")) or not _valid_digest(objective.get("replay_identity")) or objective.get("budget_units", 0) <= 0 or objective.get("raw_data_local") is not True or objective.get("boundary") != PRECLINICAL_BOUNDARY or not candidates:
        raise ResearchContractError("objective identity, required closure, digest, budget, locality, or boundary is invalid")
    rows = sorted((dict(row) for row in candidates), key=lambda row: (-row.get("power_milli", 0), -row.get("sample_size", 0), row.get("candidate_id", "")))
    ids = sorted(row.get("candidate_id", "") for row in rows)
    if not all(ids) or len(ids) != len(set(ids)):
        raise ResearchContractError("candidate identifiers must be unique and non-empty")
    ranked = [row["candidate_id"] for row in rows]; executable: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); evidence: set[str] = set(); plans: set[str] = set()
    for row in rows:
        cid = row["candidate_id"]; evidence.add(f"{cid}:{row.get('evidence_state', 'unknown')}"); omissions.update(f"{cid}:{item}" for item in row.get("omissions", [])); uncertainty.update(f"{cid}:{item}" for item in row.get("uncertainty", []))
        if row.get("negative_result") or row.get("evidence_state") == "contradicted": negative.add(f"{cid}:negative-result")
        missing = sorted(set(objective.get("required_factor_order", [])) - set(row.get("factor_order", []))); omissions.update(f"{cid}:missing-factor:{factor}" for factor in missing)
        hard = not row.get("signed") or not row.get("comparable") or row.get("baseline_digest") != objective["baseline_digest"] or row.get("replay_identity") != objective["replay_identity"] or row.get("semantic_profile") != objective["semantic_profile"] or row.get("expected_cost_units", 0) > objective["budget_units"]
        if hard: blocked.add(cid); uncertainty.add(f"{cid}:authorization-comparability-replay-or-budget-blocked")
        elif missing or row.get("evidence_state") not in {"proven", "supported"} or row.get("power_milli", 0) < 800 or not _valid_digest(row.get("design_digest")) or not _valid_digest(row.get("provenance_digest")): unresolved.add(cid); uncertainty.add(f"{cid}:power-evidence-or-closure-incomplete")
        else: executable.add(cid); plans.add(f"{cid}:n{row.get('sample_size', 0)}:power{row.get('power_milli', 0)}")
    missing_candidate = sorted(set(objective["required_candidate_order"]) - set(ids)); omissions.update(f"request:missing-candidate:{cid}" for cid in missing_candidate); missing_factor = sorted(factor for factor in objective.get("required_factor_order", []) if not any(factor in row.get("factor_order", []) for row in rows)); global_block = not all(objective.get(key) is True for key in ("policy_allow", "protected_closure", "raw_data_local"))
    if global_block: blocked.update(ids); executable.clear(); unresolved.clear(); omissions.add("request:policy-protected-closure-or-locality-blocked")
    disposition = "blocked" if global_block or (not executable and not unresolved) else "partial" if missing_candidate or missing_factor or blocked or unresolved else "qualified"; omissions.add("request:design-closure-not-ready") if disposition != "qualified" else None
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": objective["request_id"], "researcher_id": objective["researcher_id"], "study_program": objective["study_program"], "purpose": objective["purpose"], "scope": objective["scope"], "semantic_profile": objective["semantic_profile"], "disposition": disposition, "candidate_order": ids, "ranked_order": ranked, "executable_order": sorted(executable), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "missing_candidate_order": missing_candidate, "missing_factor_order": missing_factor, "plan_order": sorted(plans), "evidence_order": sorted(evidence), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "baseline_digest": objective["baseline_digest"], "replay_identity": objective["replay_identity"], "budget_units": objective["budget_units"], "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    plan_digest = _digest(payload); payload["plan_digest"] = plan_digest; payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"executable-experiment-design-5:{objective['request_id']}", "content_type": CONTENT_TYPE, "content_hash": plan_digest, "semantic_loss": [{"field": item, "reason": "design closure or evidence omission", "severity": "unknown"} for item in sorted(omissions)], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"view:design-plan:{objective['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    validate_executable_experiment_design(payload)
    return payload


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "experiment_design_research_workbench_manifest", "compile_experiment_design_workbench", "validate_executable_experiment_design"]
