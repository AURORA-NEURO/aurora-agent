"""Local single-study WeaveLang execution assurance (``AFA-weavelang-P12-F25``).

This SDK surface verifies a typed workflow graph and returns an ``ExecutionRun7``-compatible
receipt. It is intentionally advisory/local: no process, connector, network, instrument, or
clinical operation is invoked by the verifier.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-weavelang-P12-F25"
CONTRACT_VERSION = "weavelang-local-computational-execution-assurance-harness/1.0"
INPUT_SCHEMA = "ResearchWorkflowSpec1@1"
OUTPUT_SCHEMA = "ExecutionRun7@1"
CONTENT_TYPE = "application/vnd.aurora.weavelang-execution-run-7+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class ExecutionRunReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; workflow_id: str; actor_id: str; disposition: str
    node_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    missing_dependency_order: tuple[str, ...]; cycle_order: tuple[str, ...]; unauthorized_effect_order: tuple[str, ...]; budget_exhausted_order: tuple[str, ...]
    omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]; replay_identity: str; run_digest: str
    artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not all(isinstance(v, str) and v.strip() for v in (self.request_id, self.workflow_id, self.actor_id)) or not self.node_order or not self.effect_receipts:
            raise ResearchContractError("execution identity, graph, locality, or effects are incomplete")
        for values in (self.node_order, self.selected_order, self.unresolved_order, self.blocked_order, self.missing_dependency_order, self.cycle_order, self.unauthorized_effect_order, self.budget_exhausted_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.effect_receipts):
            if not _canonical(list(values)): raise ResearchContractError("execution ordering is not canonical")
        ids = set(self.node_order); parts = list(self.selected_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(parts) != ids or len(parts) != len(ids): raise ResearchContractError("execution node states do not partition the graph")
        if not all(_digest(v) for v in (self.replay_identity, self.run_digest, self.artifact.get("content_hash"))): raise ResearchContractError("execution digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("execution artifact metadata is invalid")
        expected = [f"verify:weavelang-execution:{self.workflow_id}"] if self.disposition == "qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts) != expected: raise ResearchContractError("execution effect is invalid")


def assure_computational_execution(*, request: Mapping[str, Any]) -> ExecutionRunReceipt:
    if any(not str(request.get(k, "")).strip() for k in ("request_id", "workflow_id", "actor_id")) or request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or not request.get("nodes") or not _digest(request.get("replay_identity")) or not _canonical([str(v) for v in request.get("adversarial_events", [])]):
        raise ResearchContractError("workflow identity, graph, replay, locality, or boundary is invalid")
    nodes = sorted(request["nodes"], key=lambda item: str(item.get("node_id", ""))); node_order = [str(n.get("node_id", "")) for n in nodes]; ids = set(node_order)
    if not all(node_order) or len(ids) != len(node_order): raise ResearchContractError("workflow node identities must be unique and non-empty")
    missing: set[str] = set(); cycles: set[str] = set(); unauthorized: set[str] = set(); budget_exhausted: set[str] = set(); selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    allowed = {"read-local", "execute-local-computation", "write-local-artifact"}; by_id = {str(n["node_id"]): n for n in nodes}
    for node in nodes:
        nid = str(node["node_id"]); deps = [str(v) for v in node.get("dependency_order", [])]
        if not _canonical(deps) or any(not _digest(node.get(k)) for k in ("input_digest", "output_digest")): raise ResearchContractError(f"workflow node {nid} is malformed")
        missing.update(f"{nid}->{dep}" for dep in deps if dep not in ids)
        if str(node.get("effect", "")) not in allowed or node.get("permitted") is not True: unauthorized.add(nid)
        cost = float(node.get("cost", 0)); budget = float(request.get("budgets", {}).get(str(node.get("resource", "")), 0))
        if cost < 0 or budget < cost: budget_exhausted.add(nid)
        state = str(node.get("evidence_state", "unknown"))
        if state == "contradicted": blocked.add(nid); negative.add(f"{nid}:contradicted")
        elif state == "unknown": unresolved.add(nid); uncertainty.add(f"{nid}:unknown")
        elif state == "unmeasured": unresolved.add(nid); omissions.add(f"{nid}:unmeasured")
    marks: dict[str, int] = {}
    def visit(nid: str) -> None:
        if marks.get(nid) == 1: cycles.add(nid); return
        if marks.get(nid) == 2: return
        marks[nid] = 1
        for dep in by_id[nid].get("dependency_order", []):
            if str(dep) in by_id: visit(str(dep))
        marks[nid] = 2
    for nid in node_order: visit(nid)
    global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("raw_data_local") is not True or bool(request.get("adversarial_events"))
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("raw_data_local") is not True: negative.add("request:raw-data-locality-violation")
    negative.update(f"adversarial:{v}" for v in request.get("adversarial_events", []))
    blocked.update(unauthorized | budget_exhausted | cycles | {v.split("->", 1)[0] for v in missing})
    if cycles: blocked.update(node_order)
    if global_block: blocked.update(node_order); selected.clear(); unresolved.clear(); omissions.add("request:weavelang-release-gate-blocked")
    if not global_block: selected.update(set(node_order) - blocked - unresolved)
    disposition = "blocked" if global_block or blocked else "unresolved" if unresolved or missing else "qualified"
    selected_order, unresolved_order, blocked_order = sorted(selected), sorted(unresolved), sorted(blocked)
    missing_order, cycle_order, unauthorized_order, budget_order = sorted(missing), sorted(cycles), sorted(unauthorized), sorted(budget_exhausted)
    omission_order, uncertainty_order, negative_order = sorted(omissions), sorted(uncertainty), sorted(negative); effects = [f"verify:weavelang-execution:{request['workflow_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": str(request["request_id"]), "workflow_id": str(request["workflow_id"]), "actor_id": str(request["actor_id"]), "disposition": disposition, "node_order": node_order, "selected_order": selected_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "missing_dependency_order": missing_order, "cycle_order": cycle_order, "unauthorized_effect_order": unauthorized_order, "budget_exhausted_order": budget_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_order, "replay_identity": str(request["replay_identity"]), "effect_receipts": effects, "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"weavelang-execution-run:{request['workflow_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}
    receipt = ExecutionRunReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["workflow_id"]), str(request["actor_id"]), disposition, tuple(node_order), tuple(selected_order), tuple(unresolved_order), tuple(blocked_order), tuple(missing_order), tuple(cycle_order), tuple(unauthorized_order), tuple(budget_order), tuple(omission_order), tuple(uncertainty_order), tuple(negative_order), str(request["replay_identity"]), digest, artifact, tuple(effects), True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


def validate_computational_execution_receipt(value: Mapping[str, Any]) -> ExecutionRunReceipt:
    tuple_keys = ("node_order", "selected_order", "unresolved_order", "blocked_order", "missing_dependency_order", "cycle_order", "unauthorized_effect_order", "budget_exhausted_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
    fields = [value.get(k) for k in ("schema_version", "contract_version", "feature_id", "request_id", "workflow_id", "actor_id", "disposition")]
    receipt = ExecutionRunReceipt(*fields, **{k: tuple(value.get(k, [])) for k in tuple_keys}, replay_identity=value.get("replay_identity"), run_digest=value.get("run_digest"), artifact=value.get("artifact", {}), raw_data_local=value.get("raw_data_local"), boundary=value.get("boundary"))
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "ExecutionRunReceipt", "assure_computational_execution", "validate_computational_execution_receipt"]
