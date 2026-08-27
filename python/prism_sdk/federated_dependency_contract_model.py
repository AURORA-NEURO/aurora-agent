"""Federated continual dependency-composition contract model for ``AFA-interweave-P27-F08``."""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-interweave-P27-F08"
CONTRACT_VERSION = "interweave-federated-continual-dependency-composition-contract-model/1.0"
INPUT_SCHEMA = "InterweaveDependencyContract5@1"
OUTPUT_SCHEMA = "CapabilityComposition6@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class FederatedDependencyCompositionReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; semantic_profile: str; protocol_version: str; disposition: str
    requested_capability_order: tuple[str, ...]; selected_capability_order: tuple[str, ...]; missing_capability_order: tuple[str, ...]; incompatible_capability_order: tuple[str, ...]; cycle_order: tuple[str, ...]; unresolved_capability_order: tuple[str, ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; composition_digest: str; replay_identity: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; aggregate_only: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.federation_id, self.purpose, self.semantic_profile, self.protocol_version)) or not self.requested_capability_order or not self.effect_receipts:
            raise ResearchContractError("dependency composition identity, locality, aggregate boundary, capabilities, or effects are incomplete")
        for values in (self.requested_capability_order, self.selected_capability_order, self.missing_capability_order, self.incompatible_capability_order, self.cycle_order, self.unresolved_capability_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("dependency composition ordering is not canonical")
        if len(set(self.requested_capability_order)) != len(self.requested_capability_order): raise ResearchContractError("requested capabilities are duplicated")
        outcomes = list(self.selected_capability_order) + list(self.missing_capability_order) + list(self.incompatible_capability_order) + list(self.unresolved_capability_order)
        if len(set(outcomes)) != len(outcomes) or not set(self.requested_capability_order).issubset(outcomes): raise ResearchContractError("dependency composition outcomes do not cover requests")
        for digest in (self.composition_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not _digest(digest): raise ResearchContractError("dependency composition digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.capability-composition+json": raise ResearchContractError("dependency composition artifact type is invalid")
        if any(not effect.startswith("compose:capability-contract:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("dependency composition effect is outside no-execution gate")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def assure_federated_dependency_composition(*, request: Mapping[str, Any]) -> FederatedDependencyCompositionReceipt:
    required = ("request_id", "federation_id", "purpose", "semantic_profile", "protocol_version")
    if any(not str(request.get(field, "")).strip() for field in required) or not request.get("requested_capability_order") or not _canonical([str(item) for item in request["requested_capability_order"]]) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("boundary") != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("dependency composition request identity, capability order, locality, aggregate boundary, budget, or boundary is invalid")
    requested = [str(item) for item in request["requested_capability_order"]]
    if len(set(requested)) != len(requested): raise ResearchContractError("requested capabilities must be unique")
    declarations = {}
    for item in request.get("declarations", []):
        cid = str(item.get("capability_id", "")); deps = [str(dep) for dep in item.get("dependency_order", [])]; effects = [str(effect) for effect in item.get("effect_order", [])]
        if not cid.strip() or not _canonical(deps) or not _canonical(effects) or str(item.get("semantic_profile", "")) != str(request["semantic_profile"]) or str(item.get("determinism", "")) != "byte-stable" or not _digest(item.get("artifact_digest")) or not _digest(item.get("provenance_digest")) or item.get("permitted") is not True or item.get("raw_data_local") is not True or cid in declarations:
            raise ResearchContractError("capability declarations are duplicated, non-canonical, unbound, unsigned, or non-local")
        declarations[cid] = item
    missing: set[str] = set(); incompatible: set[str] = set(); unresolved: set[str] = set(); cycles: set[str] = set(); selected: set[str] = set(); state: dict[str, int] = {}
    def visit(cid: str) -> None:
        if state.get(cid) == 1: cycles.add(cid); return
        if state.get(cid) == 2: return
        if cid not in declarations: missing.add(cid); return
        state[cid] = 1; item = declarations[cid]
        for dep in item.get("dependency_order", []): visit(str(dep))
        state[cid] = 2
        evidence = str(item.get("evidence_state", "unknown"))
        if cid in cycles or evidence == "contradicted": incompatible.add(cid)
        elif evidence in {"unknown", "speculative"} or item.get("uncertainty") or item.get("omissions"): unresolved.add(cid)
        else: selected.add(cid)
    for cid in requested: visit(cid)
    for cid in list(selected):
        dependencies = {str(dep) for dep in declarations[cid].get("dependency_order", [])}
        if dependencies & (missing | cycles | incompatible | unresolved):
            selected.remove(cid); unresolved.add(cid)
    omissions = {f"capability:{item['capability_id']}:{note}" for item in declarations.values() for note in item.get("omissions", [])}; uncertainty = {f"capability:{item['capability_id']}:{note}" for item in declarations.values() for note in item.get("uncertainty", [])}; negative = {f"capability:{item['capability_id']}:negative-result" for item in declarations.values() if item.get("negative_result")}
    for cid in missing: omissions.add(f"missing-capability:{cid}"); uncertainty.add(f"missing-capability:{cid}")
    for cid in cycles: omissions.add(f"dependency-cycle:{cid}")
    if request.get("policy_allow") is not True: incompatible.add("workflow:policy-denied")
    if request.get("protected_closure") is not True: incompatible.add("workflow:protected-closure-incomplete")
    if request.get("signed_approval") is not True: incompatible.add("workflow:signed-approval-missing")
    if request.get("federation_approved") is not True: incompatible.add("workflow:federation-approval-missing")
    for event in request.get("adversarial_events", []): incompatible.add(f"adversarial:{event}"); omissions.add(f"workflow:adversarial:{event}")
    disposition = "blocked" if incompatible or cycles or request.get("adversarial_events") else "unresolved" if missing or unresolved or uncertainty else "qualified"
    selected_order = sorted(selected); missing_order = sorted(missing); incompatible_order = sorted(incompatible); cycle_order = sorted(cycles); unresolved_order = sorted(unresolved)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "protocol_version": request["protocol_version"], "requested_capability_order": requested, "selected_capability_order": selected_order, "missing_capability_order": missing_order, "incompatible_capability_order": incompatible_order, "cycle_order": cycle_order, "unresolved_capability_order": unresolved_order, "disposition": disposition, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"interweave-capability-composition:{request['request_id']}", "content_type": "application/vnd.aurora.capability-composition+json", "content_hash": digest, "semantic_loss": [], "provenance": [{"source_id": request["federation_id"], "relation": "federated-capability-composition", "digest": digest}], "boundary": PRECLINICAL_BOUNDARY}
    receipt = FederatedDependencyCompositionReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["semantic_profile"]), str(request["protocol_version"]), disposition, tuple(requested), tuple(selected_order), tuple(missing_order), tuple(incompatible_order), tuple(cycle_order), tuple(unresolved_order), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), digest, str(next(iter(declarations.values()))["artifact_digest"] if declarations else _hash("empty-composition")), artifact, (f"compose:capability-contract:{request['request_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedDependencyCompositionReceipt", "assure_federated_dependency_composition"]
