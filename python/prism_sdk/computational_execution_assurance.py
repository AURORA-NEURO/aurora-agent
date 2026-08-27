"""Python parity adapter for ``AFA-cli-P12-F28`` execution assurance."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-cli-P12-F28"
CONTRACT_VERSION = "cli-federated-continual-computational-execution-assurance/1.0"
INPUT_SCHEMA = "ResearchWorkflowSpec4@1"
OUTPUT_SCHEMA = "ExecutionRun7@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class ExecutionAssuranceReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; run_id: str; workflow_id: str; scope: str; disposition: str
    plan_order: tuple[str, ...]; topological_order: tuple[str, ...]; completed_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]; cycle_order: tuple[str, ...]; missing_dependency_order: tuple[str, ...]; compensation_order: tuple[str, ...]; decisions: tuple[Mapping[str, Any], ...]; checkpoint_digest: str; replay_identity: str; run_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; federation_export: str; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.federation_export != "aggregate-digest-only" or self.raw_data_local is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.run_id, self.workflow_id, self.scope)) or not self.plan_order or len(self.topological_order) > len(self.plan_order) or len(self.decisions) != len(self.plan_order) or not self.effect_receipts: raise ResearchContractError("execution identity, locality, plan, decisions, or effects are incomplete")
        for values in (self.plan_order, self.completed_order, self.unresolved_order, self.blocked_order, self.cycle_order, self.missing_dependency_order, self.compensation_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("execution ordering is not canonical")
        if tuple(str(item.get("node_id", "")) for item in self.decisions) != self.plan_order: raise ResearchContractError("execution decisions do not match plan order")
        partition = list(self.completed_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(partition) != set(self.plan_order) or len(partition) != len(set(partition)): raise ResearchContractError("execution dispositions do not partition plan")
        if any(not effect.startswith("verify:execution-plan:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("execution effect is outside verification gate")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.replay_identity, self.run_digest, self.artifact.get("content_hash"))): raise ResearchContractError("execution digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.execution-run+json": raise ResearchContractError("execution artifact type is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def assure_computational_execution(*, request: Mapping[str, Any]) -> ExecutionAssuranceReceipt:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "run_id", "workflow_id", "scope")) or request.get("plan_schema") != INPUT_SCHEMA or not request.get("nodes") or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("federated_summary_only") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")): raise ResearchContractError("execution identity, bounds, locality, aggregate mode, replay, or boundary is invalid")
    nodes = sorted(request["nodes"], key=lambda item: str(item.get("node_id", ""))); ids = [str(node.get("node_id", "")) for node in nodes]
    if not all(ids) or len(set(ids)) != len(ids) or any(not _canonical([str(dep) for dep in node.get("dependency_order", [])]) for node in nodes): raise ResearchContractError("execution identifiers or dependency orders are invalid")
    plan_set = set(ids); indegree = {node_id: len(node.get("dependency_order", [])) for node_id, node in zip(ids, nodes)}; children: dict[str, list[str]] = {}
    for node in nodes:
        for dep in node.get("dependency_order", []): children.setdefault(str(dep), []).append(str(node["node_id"]))
    queue = sorted(node_id for node_id in ids if indegree[node_id] == 0); topological: list[str] = []
    while queue:
        node_id = queue.pop(0); topological.append(node_id)
        for child in sorted(children.get(node_id, [])):
            indegree[child] -= 1
            if indegree[child] == 0: queue.append(child); queue.sort()
    topological_set = set(topological); cycle = sorted(node_id for node in ids if node_id not in topological_set and all(str(dep) in plan_set for dep in next(item for item in nodes if str(item["node_id"]) == node_id).get("dependency_order", [])))
    global_failed = {gate for gate, failed in (("policy", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("raw-data-locality", request.get("raw_data_local") is not True), ("aggregate-only", request.get("federated_summary_only") is not True), ("adversarial-input", bool(request.get("adversarial_events")))) if failed}
    completed: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; missing: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []; spent = 0; allowed = {"compute-local", "read-local", "write-artifact"}
    for node in nodes:
        node_id = str(node["node_id"]); failed = set(global_failed); pending: set[str] = set(); dependencies = [str(dep) for dep in node.get("dependency_order", [])]
        for dep in sorted(set(dependencies) - plan_set): pending.add("missing-dependency"); missing.add(f"{node_id}:{dep}"); omissions.add(f"{node_id}:missing-dependency:{dep}")
        if node_id in cycle: failed.add("dependency-cycle")
        if str(node.get("effect_kind", "")) not in allowed: failed.add("effect-not-local-allow-listed")
        if node.get("replay_identity") != request["replay_identity"]: failed.add("replay-identity")
        if not node.get("artifact_digest"): pending.add("artifact-digest-missing"); omissions.add(f"{node_id}:artifact-digest-missing")
        if not node.get("provenance_digest"): pending.add("provenance-missing"); omissions.add(f"{node_id}:provenance-missing")
        for item in node.get("omissions", []): pending.add("node-omissions"); omissions.add(f"{node_id}:{item}")
        for item in node.get("uncertainty", []): pending.add("node-uncertainty"); uncertainty.add(f"{node_id}:{item}")
        state = str(node.get("evidence_state", "unknown"));
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state-not-qualified"); uncertainty.add(f"{node_id}:evidence-state")
        negative.add(f"{node_id}:execution-not-started"); cost = int(node.get("estimated_cost", 0))
        if cost > int(request["budget_units"]) - spent: pending.add("budget-ceiling"); omissions.add(f"{node_id}:budget-ceiling")
        else: spent += cost
        disposition = "blocked" if failed else "unresolved" if pending else "completed"; (blocked if disposition == "blocked" else unresolved if disposition == "unresolved" else completed).append(node_id); decisions.append({"node_id": node_id, "effect_kind": str(node.get("effect_kind", "")), "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending)})
        if failed: semantic_loss.append({"field": f"node:{node_id}", "reason": "execution node failed a release gate", "severity": "decision_relevant"})
    completed = sorted(set(completed)); unresolved = sorted(set(unresolved)); blocked = sorted(set(blocked)); compensation = tuple(sorted({f"retain:{node_id}:no-dispatch" for node_id in unresolved + blocked})); disposition = "blocked" if global_failed or blocked else "unresolved" if unresolved or len(completed) != len(ids) else "qualified"; checkpoint_digest = _hash({"run_id": request["run_id"], "workflow_id": request["workflow_id"], "plan_order": ids, "topological_order": topological, "replay_identity": request["replay_identity"]}); payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "run_id": request["run_id"], "workflow_id": request["workflow_id"], "scope": request["scope"], "disposition": disposition, "plan_order": ids, "topological_order": topological, "completed_order": completed, "unresolved_order": unresolved, "blocked_order": blocked, "cycle_order": cycle, "missing_dependency_order": sorted(missing), "compensation_order": list(compensation), "decisions": decisions, "checkpoint_digest": checkpoint_digest, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}; run_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"execution-run:{request['run_id']}", "content_type": "application/vnd.aurora.execution-run+json", "content_hash": run_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": str(request["run_id"]), "relation": "computational-execution-assurance", "digest": run_digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = ExecutionAssuranceReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["run_id"]), str(request["workflow_id"]), str(request["scope"]), disposition, tuple(ids), tuple(topological), tuple(completed), tuple(unresolved), tuple(blocked), tuple(cycle), tuple(sorted(missing)), compensation, tuple(decisions), checkpoint_digest, str(request["replay_identity"]), run_digest, tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, (f"verify:execution-plan:{request['run_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, "aggregate-digest-only", PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "ExecutionAssuranceReceipt", "assure_computational_execution"]
