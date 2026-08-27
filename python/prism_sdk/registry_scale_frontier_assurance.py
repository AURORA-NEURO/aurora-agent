"""Multimodal registry-scale frontier assurance (``AFA-registry-P29-F26``)."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-registry-P29-F26"; CONTRACT_VERSION = "registry-multimodal-scale-frontier-assurance-harness/1.0"; INPUT_SCHEMA = "RegistryScaleWorkload2@1"; OUTPUT_SCHEMA = "RegistryCapacityReport7@1"; CONTENT_TYPE = "application/vnd.aurora.registry-capacity-report-7+json"
def _hash(v: Any) -> str: return hashlib.sha256(json.dumps(v, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(v: Any) -> bool: return isinstance(v, str) and re.fullmatch(r"[0-9a-f]{64}", v) is not None
def _canonical(v: list[str]) -> bool: return v == sorted(set(v))

@dataclass(frozen=True)
class RegistryCapacityReport:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; registry_id: str; semantic_profile: str; disposition: str
    study_order: tuple[str, ...]; qualified_study_order: tuple[str, ...]; unresolved_study_order: tuple[str, ...]; blocked_study_order: tuple[str, ...]; missing_modality_order: tuple[str, ...]; capacity_exceeded_order: tuple[str, ...]
    omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]; observed_studies: int; observed_artifacts: int; observed_bytes: int; observed_operation_units: int
    replay_identity: str; report_digest: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for k, v in value.items():
            if isinstance(v, tuple): value[k] = list(v)
        return value
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not self.request_id.strip() or not self.registry_id.strip() or not self.semantic_profile.strip() or not self.study_order or not self.effect_receipts: raise ResearchContractError("registry capacity identity, locality, studies, or effects are incomplete")
        for values in (self.study_order, self.qualified_study_order, self.unresolved_study_order, self.blocked_study_order, self.missing_modality_order, self.capacity_exceeded_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.effect_receipts):
            if not _canonical(list(values)): raise ResearchContractError("registry capacity ordering is not canonical")
        ids = set(self.study_order); parts = list(self.qualified_study_order) + list(self.unresolved_study_order) + list(self.blocked_study_order)
        if set(parts) != ids or len(parts) != len(ids): raise ResearchContractError("registry study states do not partition workload")
        if not all(_digest(v) for v in (self.replay_identity, self.report_digest, self.artifact.get("content_hash"))): raise ResearchContractError("registry capacity digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("registry capacity artifact metadata is invalid")
        expected = [f"measure:registry-capacity:{self.registry_id}"] if self.disposition == "qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts) != expected: raise ResearchContractError("registry capacity effect is invalid")

def assure_registry_scale_frontier(*, request: Mapping[str, Any]) -> RegistryCapacityReport:
    if any(not str(request.get(k, "")).strip() for k in ("request_id", "registry_id", "semantic_profile")) or request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or not request.get("studies") or not request.get("required_modality_order") or not _canonical([str(v) for v in request["required_modality_order"]]) or any(int(request.get(k, 0)) <= 0 for k in ("max_studies", "max_artifacts", "max_bytes", "max_operation_units")) or not _digest(request.get("replay_identity")) or not _canonical([str(v) for v in request.get("adversarial_events", [])]): raise ResearchContractError("registry workload identity, modality closure, capacity, replay, locality, or boundary is invalid")
    studies = sorted(request["studies"], key=lambda item: str(item.get("study_id", ""))); study_order = [str(s.get("study_id", "")) for s in studies];
    if not all(study_order) or len(set(study_order)) != len(study_order): raise ResearchContractError("study identities must be unique and non-empty")
    required = {str(v) for v in request["required_modality_order"]}; qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); missing: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    artifacts = sum(int(s.get("artifact_count", 0)) for s in studies); bytes_total = sum(int(s.get("bytes", 0)) for s in studies); units = sum(int(s.get("operation_units", 0)) for s in studies)
    for s in studies:
        sid = str(s["study_id"]); modalities = {str(v) for v in s.get("modality_order", [])}; missing.update(f"{sid}:{m}" for m in required - modalities); omissions.update(f"{sid}:{v}" for v in s.get("omissions", [])); uncertainty.update(f"{sid}:{v}" for v in s.get("uncertainty", []));
        if s.get("negative_result") is True: negative.add(f"{sid}:negative-result")
        state = str(s.get("state", "unknown"))
        if state == "contradicted" or s.get("local_only") is not True or s.get("permitted") is not True: blocked.add(sid)
        elif state in {"unknown", "unmeasured"} or s.get("omissions") or s.get("uncertainty") or not required <= modalities: unresolved.add(sid)
        else: qualified.add(sid)
    capacity: set[str] = set();
    if len(studies) > int(request["max_studies"]): capacity.add("studies")
    if artifacts > int(request["max_artifacts"]): capacity.add("artifacts")
    if bytes_total > int(request["max_bytes"]): capacity.add("bytes")
    if units > int(request["max_operation_units"]): capacity.add("operation-units")
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    negative.update(f"adversarial:{v}" for v in request.get("adversarial_events", [])); global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or request.get("raw_data_local") is not True or bool(request.get("adversarial_events")) or bool(capacity)
    if global_block: blocked.update(study_order); qualified.clear(); unresolved.clear(); omissions.add("request:registry-capacity-gate-blocked")
    disposition = "blocked" if global_block or blocked else "unresolved" if unresolved else "qualified"; q, u, b = sorted(qualified), sorted(unresolved), sorted(blocked); miss, cap = sorted(missing), sorted(capacity); om, un, neg = sorted(omissions), sorted(uncertainty), sorted(negative); effects = [f"measure:registry-capacity:{request['registry_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": str(request["request_id"]), "registry_id": str(request["registry_id"]), "semantic_profile": str(request["semantic_profile"]), "disposition": disposition, "study_order": study_order, "qualified_study_order": q, "unresolved_study_order": u, "blocked_study_order": b, "missing_modality_order": miss, "capacity_exceeded_order": cap, "omission_order": om, "uncertainty_order": un, "negative_evidence_order": neg, "observed_studies": len(studies), "observed_artifacts": artifacts, "observed_bytes": bytes_total, "observed_operation_units": units, "replay_identity": str(request["replay_identity"]), "effect_receipts": effects, "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}; digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"registry-capacity-report:{request['registry_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}; receipt = RegistryCapacityReport(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["registry_id"]), str(request["semantic_profile"]), disposition, tuple(study_order), tuple(q), tuple(u), tuple(b), tuple(miss), tuple(cap), tuple(om), tuple(un), tuple(neg), len(studies), artifacts, bytes_total, units, str(request["replay_identity"]), digest, artifact, tuple(effects), True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "RegistryCapacityReport", "assure_registry_scale_frontier"]
