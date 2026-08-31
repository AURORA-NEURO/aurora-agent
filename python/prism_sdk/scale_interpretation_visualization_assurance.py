"""Parity implementation for ``AFA-scale-P14-F28``.

The assurance boundary classifies caller-supplied interpretation candidates; it never renders
raw data, fits models, or exports institution-local payloads.
"""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-scale-P14-F28"
CONTRACT_VERSION = "scale-federated-continual-interpretation-visualization-assurance-harness/1.0"
INPUT_SCHEMA = "EvidenceBackedResult4@1"
OUTPUT_SCHEMA = "InteractiveInterpretation7@1"
CONTENT_TYPE = "application/vnd.aurora.scale-interactive-interpretation-7+json"
# The scale assurance surface emits a mapping rather than the runtime wrapper class.  Keep a
# public structural alias so package-level imports remain usable across both representations.

@dataclass(frozen=True)
class InteractiveInterpretation7:
    """Typed wrapper for the scale interpretation receipt.

    The JSON-facing assurance function intentionally returns a plain mapping for parity with
    the TypeScript SDK.  This wrapper preserves the package-level typed API used by callers that
    want an object with the same ``to_dict``/``validate`` affordances as other SDK receipts.
    """

    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        validate_interpretation_visualization(self.value)

    def digest(self) -> str:
        self.validate()
        return _hash(self.value)

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(value: list[str]) -> bool:
    return value == sorted(set(value))

def interpretation_visualization_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "scale", "consumers": ["laboratory automation engineer", "research workflow operator", "visualization steward"], "behavior": "verify federated continual multimodal interpretation and visualization candidates with comparability, evidence, provenance, replay, policy, and locality gates", "value": "prevents incomplete or non-comparable interpretations from being rendered or shared as qualified research artifacts", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["execute:local-computation", "write:local-artifact"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

def _validate_request(request: Mapping[str, Any]) -> None:
    if request.get("schema_version") != INPUT_SCHEMA or any(not isinstance(request.get(k), str) or not request[k].strip() for k in ("request_id", "consumer", "purpose", "target_scope", "semantic_profile")) or not request.get("required_panel_order") or not _ordered(request["required_panel_order"]) or not request.get("minimum_comparability_milli") or request["minimum_comparability_milli"] > 1000 or not _digest(request.get("replay_identity")) or not request.get("aggregate_only") or not request.get("raw_data_local") or request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("candidates"):
        raise ResearchContractError("interpretation identity, panel, replay, locality, bounds, or boundary is invalid")
    ids: set[str] = set()
    for candidate in request["candidates"]:
        if not isinstance(candidate.get("interpretation_id"), str) or not candidate["interpretation_id"].strip() or candidate["interpretation_id"] in ids or not _digest(candidate.get("artifact_digest")) or not _digest(candidate.get("provenance_digest")) or candidate.get("replay_identity") != request["replay_identity"] or not isinstance(candidate.get("comparability_milli"), int) or candidate["comparability_milli"] > 1000 or not _ordered(candidate.get("omission_order", [])) or not _ordered(candidate.get("uncertainty_order", [])):
            raise ResearchContractError("candidate identity, digest, replay, comparability, or ordering is invalid")
        ids.add(candidate["interpretation_id"])

def validate_interpretation_visualization(output: Mapping[str, Any]) -> None:
    artifact = output.get("artifact", {})
    if output.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or output.get("contract_version") != CONTRACT_VERSION or output.get("feature_id") != FEATURE_ID or output.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("boundary") != PRECLINICAL_BOUNDARY or artifact.get("content_type") != CONTENT_TYPE or output.get("raw_data_local") is not True or output.get("aggregate_only") is not True or output.get("disposition") not in {"qualified", "partial", "blocked"} or not output.get("candidate_order") or not output.get("panel_order") or not output.get("effect_receipts"):
        raise ResearchContractError("interpretation identity, locality, panel, disposition, or effects are incomplete")
    fields = ("candidate_order", "panel_order", "qualified_order", "unresolved_order", "blocked_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
    if any(not _ordered(output.get(k, [])) for k in fields):
        raise ResearchContractError("interpretation ordering is not canonical")
    ids = set(output["candidate_order"]); parts = sum((output.get(k, []) for k in ("qualified_order", "unresolved_order", "blocked_order")), [])
    if len(ids) != len(output["candidate_order"]) or len(parts) != len(ids) or set(parts) != ids:
        raise ResearchContractError("interpretation candidate states do not partition")
    if not _digest(output.get("replay_identity")) or not _digest(output.get("interpretation_digest")) or artifact.get("content_hash") != output.get("interpretation_digest") or any(not _digest(v) for v in artifact.get("provenance_digests", [])):
        raise ResearchContractError("interpretation digest is inconsistent")
    if any(v != "block:unsafe-release" and not v.startswith("render:interpretation:") for v in output["effect_receipts"]):
        raise ResearchContractError("interpretation effect is outside assurance gate")
    if output["disposition"] == "qualified" and output["effect_receipts"] != [f"render:interpretation:{output['request_id']}"]:
        raise ResearchContractError("qualified interpretation effect is invalid")
    if output["disposition"] != "qualified" and output["effect_receipts"] != ["block:unsafe-release"]:
        raise ResearchContractError("non-qualified interpretation must block")

def assure_interpretation_visualization(request: Mapping[str, Any]) -> dict[str, Any]:
    _validate_request(request)
    candidates = sorted((dict(c) for c in request["candidates"]), key=lambda c: c["interpretation_id"])
    candidate_order = [c["interpretation_id"] for c in candidates]; qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for c in candidates:
        ident = c["interpretation_id"]; provenance.add(c["provenance_digest"]); omissions.update(f"{ident}:{v}" for v in c.get("omission_order", [])); uncertainty.update(f"{ident}:{v}" for v in c.get("uncertainty_order", []))
        if c.get("negative_result"): negative.add(ident)
        hard = not c.get("policy_allowed") or not c.get("local_only") or not c.get("protected_closure") or c.get("target_scope") != request["target_scope"] or c.get("semantic_profile") != request["semantic_profile"] or c.get("comparability_milli", 0) < request["minimum_comparability_milli"] or not c.get("visualization_ready")
        if hard: blocked.add(ident); omissions.add(f"{ident}:interpretation-integrity-or-comparability")
        elif c.get("evidence_state") in {"contradicted", "unknown", "speculative"}: unresolved.add(ident); uncertainty.add(f"{ident}:evidence-state")
        else: qualified.add(ident)
    for ok, label in ((request.get("policy_allowed"), "workflow:policy-denied"), (request.get("protected_closure"), "workflow:protected-closure-incomplete"), (request.get("signed_approval"), "workflow:signed-approval-missing"), (request.get("adversarial_clear"), "workflow:adversarial-gate-failed")):
        if not ok: omissions.add(label)
    global_block = not all(request.get(k) is True for k in ("policy_allowed", "protected_closure", "signed_approval", "adversarial_clear"))
    disposition = "blocked" if global_block or blocked else "partial" if unresolved or not qualified else "qualified"
    if global_block: blocked.update(candidate_order); qualified.clear(); unresolved.clear()
    if disposition != "qualified": omissions.add("workflow:interpretation-closure-not-ready")
    payload = {"candidate_order": candidate_order, "panel_order": list(request["required_panel_order"]), "qualified_order": sorted(qualified), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"]}
    digest = _hash(payload)
    output = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "consumer": request["consumer"], "purpose": request["purpose"], "target_scope": request["target_scope"], "semantic_profile": request["semantic_profile"], "disposition": disposition, **payload, "interpretation_digest": digest, "artifact": {"artifact_id": f"scale-interpretation:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [] if disposition == "qualified" else ["interpretation-not-released"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": [f"render:interpretation:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    validate_interpretation_visualization(output); return output

def assure_interpretation_visualization_json(value: Mapping[str, Any]) -> dict[str, Any]: return assure_interpretation_visualization(value)

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "InteractiveInterpretation7", "interpretation_visualization_assurance_manifest", "assure_interpretation_visualization", "assure_interpretation_visualization_json", "validate_interpretation_visualization"]
