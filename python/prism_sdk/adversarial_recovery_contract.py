"""Python parity adapter for ``AFA-examples-P30-F08``."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-examples-P30-F08"
CONTRACT_VERSION = "examples-federated-continual-adversarial-recovery-contract/1.0"
INPUT_SCHEMA = "ExamplesAdversarialCase4@1"
OUTPUT_SCHEMA = "ExamplesRecoveryRecord2@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class ExamplesRecoveryRecord:
    schema_version: str; contract_version: str; feature_id: str; case_id: str; scenario_id: str; scope: str; disposition: str; event_order: tuple[str, ...]; recovered_order: tuple[str, ...]; pending_order: tuple[str, ...]; blocked_order: tuple[str, ...]; compensated_order: tuple[str, ...]; class_order: tuple[str, ...]; replay_identity: str; recovery_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or not all(str(value).strip() for value in (self.case_id, self.scenario_id, self.scope)) or not self.event_order or not self.effect_receipts or self.raw_data_local is not True or self.boundary != PRECLINICAL_BOUNDARY: raise ResearchContractError("examples recovery identity, locality, events, or effects are incomplete")
        for values in (self.event_order, self.recovered_order, self.pending_order, self.blocked_order, self.compensated_order, self.class_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("examples recovery ordering is not canonical")
        partition = list(self.recovered_order) + list(self.pending_order) + list(self.blocked_order)
        if set(partition) != set(self.event_order) or len(partition) != len(set(partition)): raise ResearchContractError("examples recovery states do not partition events")
        if any(effect not in {"retain:recovery-record", "block:unsafe-release"} for effect in self.effect_receipts): raise ResearchContractError("examples recovery effect is outside retention gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.examples-recovery-record+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("examples recovery artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def classify_adversarial_recovery(*, request: Mapping[str, Any]) -> ExamplesRecoveryRecord:
    if request.get("schema_version") != INPUT_SCHEMA or not all(str(request.get(field, "")).strip() for field in ("case_id", "scenario_id", "scope")) or not request.get("events") or not _digest(request.get("replay_identity")) or not _digest(request.get("artifact_digest")) or not _digest(request.get("provenance_digest")) or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("examples recovery case identity, bounds, locality, replay, or boundary is invalid")
    events = sorted(request["events"], key=lambda item: str(item.get("event_id", ""))); event_ids = [str(event.get("event_id", "")) for event in events]
    if not all(event_ids) or len(set(event_ids)) != len(event_ids) or any(not str(event.get("class", "")).strip() or not _digest(event.get("source_digest")) or not _digest(event.get("provenance_digest")) or int(event.get("retry_cost", 0)) <= 0 for event in events): raise ResearchContractError("recovery event identity, class, digests, or retry cost is invalid")
    recovered: list[str] = []; pending: list[str] = []; blocked: list[str] = []; compensated: set[str] = set(); classes: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); semantic_loss: list[Mapping[str, Any]] = []; spent = 0; global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True
    for event in events:
        eid = str(event["event_id"]); classes.add(str(event["class"])); negative.add(f"{eid}:{'negative-result-not-observed' if event.get('recoverable') else 'recovery-unavailable'}"); omissions.update(f"{eid}:{item}" for item in event.get("omissions", [])); uncertainty.update(f"{eid}:{item}" for item in event.get("uncertainty", [])); state = str(event.get("evidence_state", "unknown"))
        if state == "contradicted": blocked.append(eid); compensated.add(eid); semantic_loss.append({"field": f"event:{eid}", "reason": "contradicted recovery evidence cannot be replayed", "severity": "decision_relevant"}); continue
        if state in {"unknown", "speculative"}: pending.append(eid); compensated.add(eid); uncertainty.add(f"{eid}:evidence-state"); continue
        if global_block: blocked.append(eid); compensated.add(eid); continue
        cost = int(event["retry_cost"])
        if not event.get("recoverable") or cost > int(request["budget_units"]) - spent: pending.append(eid); compensated.add(eid); omissions.add(f"{eid}:recovery-budget-or-capability"); continue
        spent += cost; recovered.append(eid)
    if global_block: omissions.add("case:policy-or-protected-closure")
    disposition = "blocked" if global_block else "partial" if blocked else "unresolved" if pending else "recovered"
    recovered.sort(); pending.sort(); blocked.sort(); compensation = sorted(f"compensate:{eid}" for eid in compensated); event_order = sorted(event_ids)
    payload = {"schema_version": OUTPUT_SCHEMA, "case_id": request["case_id"], "scenario_id": request["scenario_id"], "event_order": event_order, "recovered_order": recovered, "pending_order": pending, "blocked_order": blocked, "compensated_order": compensation, "replay_identity": request["replay_identity"], "disposition": disposition}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"examples-recovery:{request['case_id']}", "content_type": "application/vnd.aurora.examples-recovery-record+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["scenario_id"], "relation": "adversarial-recovery-classification", "digest": _hash(payload)}], "boundary": PRECLINICAL_BOUNDARY}
    record = ExamplesRecoveryRecord(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["case_id"]), str(request["scenario_id"]), str(request["scope"]), disposition, tuple(event_order), tuple(recovered), tuple(pending), tuple(blocked), tuple(compensation), tuple(sorted(classes)), str(request["replay_identity"]), _hash(payload), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, ("retain:recovery-record",) if disposition == "recovered" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY)
    record.validate(); return record


__all__ = ["ExamplesRecoveryRecord", "classify_adversarial_recovery", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
