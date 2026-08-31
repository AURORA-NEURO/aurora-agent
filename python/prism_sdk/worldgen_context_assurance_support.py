"""Baseline and replication assurance for Worldgen P03 F25-F28."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .worldgen_context_compilation_support import ContextCompilationRequest, compile as compile_context

CONTENT_TYPE = "application/vnd.aurora.worldgen.context-assurance-receipt+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")

@dataclass(frozen=True)
class ContextAssuranceRequest:
    context_request: ContextCompilationRequest
    benchmark_id: str
    benchmark_digest: str
    baseline_discovery_rate_milli: int
    candidate_discovery_rate_milli: int
    required_site_order: tuple[str, ...]
    achieved_site_order: tuple[str, ...]
    minimum_site_quorum: int
    signed_approval: bool = False
    federation_approved: bool = False
    replay_identity: str = ""
    boundary: str = PRECLINICAL_BOUNDARY

@dataclass(frozen=True)
class ContextAssuranceReceipt:
    value: dict[str, Any]

    def validate(self, *, feature_id: str, contract_version: str) -> None:
        value, artifact = self.value, self.value.get("artifact", {})
        required = set(value.get("required_site_order", ()))
        parts = set(value.get("achieved_site_order", ())) | set(value.get("missing_site_order", ()))
        valid = (value.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION and value.get("contract_version") == contract_version and value.get("feature_id") == feature_id and value.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("boundary") == PRECLINICAL_BOUNDARY and artifact.get("content_type") == CONTENT_TYPE and artifact.get("raw_data") is False and value.get("raw_data_local") is True and value.get("aggregate_only") is True and required and parts == required and value.get("effect_receipts") and all(_HEX.fullmatch(value.get(key, "")) for key in ("context_digest", "benchmark_digest", "replay_identity", "assurance_digest")) and artifact.get("content_hash") == value.get("assurance_digest"))
        if not valid:
            raise ResearchContractError("context assurance identity, benchmark, quorum, locality, digests, or effects are incomplete")
        for key in ("required_site_order", "achieved_site_order", "missing_site_order", "omissions", "uncertainty", "negative_evidence", "effect_receipts"):
            values = tuple(value.get(key, ()))
            if values != tuple(sorted(set(values))):
                raise ResearchContractError("context assurance ordering is not canonical")
        if any(effect != "block:unsafe-release" and not effect.startswith("assure:worldgen-context:") for effect in value["effect_receipts"]):
            raise ResearchContractError("context assurance effect is outside qualification gate")

    def digest(self, *, feature_id: str, contract_version: str) -> str:
        self.validate(feature_id=feature_id, contract_version=contract_version)
        return _digest(self.value)

def _digest(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

def manifest(*, feature_id: str, contract_version: str, input_schema: str, scale: str, autonomy_tier: str) -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": feature_id, "version": contract_version, "owner_crate": "worldgen", "consumers": ["benchmark curator", "research program lead", "independent replication site", "downstream evaluator"], "behavior": f"assure context compilation against baseline and replication quorum at {scale}", "value": "turns context output into a reproducible, falsifiable qualification with explicit negative evidence", "input_schema": input_schema, "output_schema": "EvaluationCardContext1@1", "effects": ["assure:worldgen-context", "block:unsafe-release"], "permissions": ["read:local-evaluation-artifacts"], "determinism": "byte_stable", "autonomy_tier": autonomy_tier, "boundary": PRECLINICAL_BOUNDARY}

def assure(request: ContextAssuranceRequest, *, feature_id: str, contract_version: str, scale: str, require_approval: bool, require_federation: bool) -> ContextAssuranceReceipt:
    if (not request.benchmark_id.strip() or not request.required_site_order or tuple(request.required_site_order) != tuple(sorted(set(request.required_site_order))) or tuple(request.achieved_site_order) != tuple(sorted(set(request.achieved_site_order))) or any(site not in request.required_site_order for site in request.achieved_site_order) or request.minimum_site_quorum <= 0 or request.minimum_site_quorum > len(request.required_site_order) or not _HEX.fullmatch(request.benchmark_digest) or not _HEX.fullmatch(request.replay_identity) or request.boundary != PRECLINICAL_BOUNDARY or request.replay_identity != request.context_request.replay_identity):
        raise ResearchContractError("context assurance identity, benchmark, site quorum, locality, boundary, or replay is invalid")
    context = compile_context(request.context_request, feature_id=feature_id, contract_version=contract_version, require_federation=require_federation).value
    missing = sorted(set(request.required_site_order) - set(request.achieved_site_order))
    delta = request.candidate_discovery_rate_milli - request.baseline_discovery_rate_milli
    approval_ok = not require_approval or request.signed_approval
    federation_ok = not require_federation or request.federation_approved
    quorum_ok = len(request.achieved_site_order) >= request.minimum_site_quorum
    baseline_ok = request.candidate_discovery_rate_milli > request.baseline_discovery_rate_milli
    safe = context["disposition"] == "qualified" and approval_ok and federation_ok and quorum_ok and baseline_ok
    disposition = "blocked" if not approval_ok or not federation_ok or context["disposition"] == "blocked" else "qualified" if safe else "partial"
    omissions = list(context["omissions"])
    omissions += ([] if approval_ok else ["assurance:signed-approval-missing"]) + ([] if federation_ok else ["assurance:federation-approval-missing"]) + ([] if quorum_ok else ["assurance:replication-quorum-missing"]) + ([] if baseline_ok else ["assurance:baseline-not-beaten"])
    omissions = sorted(set(omissions))
    negative = sorted(set(context["negative_evidence"] + ([] if baseline_ok else ["assurance:candidate-did-not-beat-baseline"])))
    effects = [f"assure:worldgen-context:{request.benchmark_id}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": contract_version, "feature_id": feature_id, "request_id": request.context_request.request_id, "benchmark_id": request.benchmark_id, "disposition": disposition, "baseline_discovery_rate_milli": request.baseline_discovery_rate_milli, "candidate_discovery_rate_milli": request.candidate_discovery_rate_milli, "delta_discovery_rate_milli": delta, "required_site_order": list(request.required_site_order), "achieved_site_order": list(request.achieved_site_order), "missing_site_order": missing, "context_disposition": context["disposition"], "context_digest": context["context_digest"], "benchmark_digest": request.benchmark_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": sorted(context["uncertainty"]), "negative_evidence": negative, "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    assurance_digest = _digest(payload)
    payload["assurance_digest"] = assurance_digest
    payload["artifact"] = {"artifact_id": f"worldgen-context-evaluation:{request.benchmark_id}", "content_type": CONTENT_TYPE, "content_hash": assurance_digest, "raw_data": False, "boundary": PRECLINICAL_BOUNDARY}
    receipt = ContextAssuranceReceipt(payload)
    receipt.validate(feature_id=feature_id, contract_version=contract_version)
    return receipt

__all__ = ["CONTENT_TYPE", "ContextAssuranceRequest", "ContextAssuranceReceipt", "manifest", "assure"]
