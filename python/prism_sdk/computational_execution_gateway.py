"""Python parity adapter for ``AFA-graph-P12-F22``."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-graph-P12-F22"
CONTRACT_VERSION = "graph-multimodal-computational-execution-interoperability-gateway/1.0"
INPUT_SCHEMA = "ResearchWorkflowSpec2@1"
OUTPUT_SCHEMA = "ExecutionRun6@1"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))

@dataclass(frozen=True)
class ExecutionRun:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; workflow_id: str; scope: str; semantic_profile: str; disposition: str; plan_order: tuple[str, ...]; topological_order: tuple[str, ...]; completed_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]; cycle_order: tuple[str, ...]; missing_dependency_order: tuple[str, ...]; study_order: tuple[str, ...]; modality_order: tuple[str, ...]; exchange_order: tuple[str, ...]; replay_identity: str; checkpoint_digest: str; run_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or not all(str(value).strip() for value in (self.request_id, self.workflow_id, self.scope, self.semantic_profile)) or not self.plan_order or len(self.topological_order) > len(self.plan_order) or not self.study_order or not self.modality_order or not self.effect_receipts or self.raw_data_local is not True or self.boundary != PRECLINICAL_BOUNDARY: raise ResearchContractError("execution run identity, plan, studies, locality, or effects are incomplete")
        for values in (self.plan_order, self.completed_order, self.unresolved_order, self.blocked_order, self.cycle_order, self.missing_dependency_order, self.study_order, self.modality_order, self.exchange_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("execution run ordering is not canonical")
        partition = list(self.completed_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(partition) != set(self.plan_order) or len(partition) != len(set(partition)): raise ResearchContractError("execution states do not partition plan nodes")
        if any(not effect.startswith("exchange:permitted-artifacts:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("execution effect is outside permitted-artifact exchange gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.execution-run+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("execution artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value

def admit_computational_execution(*, request: Mapping[str, Any]) -> ExecutionRun:
    if request.get("schema_version") != INPUT_SCHEMA or not all(str(request.get(field, "")).strip() for field in ("request_id", "workflow_id", "scope", "semantic_profile")) or not request.get("nodes") or not request.get("required_study_order") or not request.get("required_modality_order") or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _canonical([str(value) for value in request["required_study_order"]]) or not _canonical([str(value) for value in request["required_modality_order"]]): raise ResearchContractError("execution request identity, bounds, locality, or canonical declarations are invalid")
    nodes = sorted(request["nodes"], key=lambda item: str(item.get("node_id", ""))); ids = [str(node.get("node_id", "")) for node in nodes]
    if not all(ids) or len(set(ids)) != len(ids) or any(not str(node.get("study_id", "")).strip() or not node.get("modality_order") or not _canonical([str(value) for value in node.get("modality_order", [])]) or not _digest(node.get("artifact_digest")) or not _digest(node.get("provenance_digest")) or not str(node.get("effect", "")).strip() or int(node.get("estimated_cost", 0)) <= 0 or str(node.get("effect")) not in request.get("permitted_effect_order", []) for node in nodes): raise ResearchContractError("execution node identity, modalities, digests, cost, or permitted effect is invalid")
    by_id = {str(node["node_id"]): node for node in nodes}; studies = {str(node["study_id"]) for node in nodes}
    if any(str(study) not in studies for study in request["required_study_order"]): raise ResearchContractError("required study is absent from graph")
    indegree = {node_id: 0 for node_id in ids}; edges: dict[str, list[str]] = {}; missing: set[str] = set()
    for node in nodes:
        for dependency in node.get("dependency_order", []):
            if dependency not in by_id: missing.add(f"{node['node_id']}:{dependency}")
            else: indegree[str(node["node_id"])] += 1; edges.setdefault(str(dependency), []).append(str(node["node_id"]))
    queue = sorted(node_id for node_id, degree in indegree.items() if degree == 0); topological: list[str] = []
    while queue:
        node_id = queue.pop(0); topological.append(node_id)
        for child in sorted(edges.get(node_id, [])):
            indegree[child] -= 1
            if indegree[child] == 0: queue.append(child); queue.sort()
    cycle = sorted(node_id for node_id, degree in indegree.items() if degree > 0); omissions = {f"missing-dependency:{item}" for item in missing}; uncertainty: set[str] = set(); negative = {f"{node_id}:negative-result-not-observed" for node_id in ids}; semantic_loss: list[Mapping[str, Any]] = []
    for node in nodes:
        nid = str(node["node_id"]); state = str(node.get("evidence_state", "unknown"));
        if state == "contradicted": semantic_loss.append({"field": f"node:{nid}", "reason": "contradicted execution evidence cannot be admitted", "severity": "decision_relevant"})
        if state in {"unknown", "speculative"}: uncertainty.add(f"{nid}:evidence-state")
        omissions.update(f"{nid}:{item}" for item in node.get("omissions", [])); uncertainty.update(f"{nid}:{item}" for item in node.get("uncertainty", []))
    global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or bool(request.get("adversarial_events"));
    if request.get("policy_allow") is not True: omissions.add("workflow:policy-denied")
    if request.get("protected_closure") is not True: omissions.add("workflow:protected-closure-incomplete")
    if request.get("signed_approval") is not True: omissions.add("workflow:signed-approval-missing")
    omissions.update(f"workflow:adversarial:{event}" for event in request.get("adversarial_events", []))
    if missing or cycle: uncertainty.add("workflow:graph-closure-incomplete")
    completed: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; spent = 0
    for nid in ids:
        node = by_id[nid]; deps = [by_id[dep] for dep in node.get("dependency_order", []) if dep in by_id]; dependency_blocked = any(str(dep.get("evidence_state")) == "contradicted" for dep in deps); dependency_uncertain = any(str(dep.get("evidence_state", "unknown")) in {"unknown", "speculative"} for dep in deps); hard = global_block or nid in cycle or str(node.get("evidence_state")) == "contradicted" or dependency_blocked; conditional = any(item.startswith(f"{nid}:") for item in missing) or str(node.get("evidence_state", "unknown")) in {"unknown", "speculative"} or dependency_uncertain or bool(node.get("omissions")) or bool(node.get("uncertainty"));
        if hard: blocked.append(nid)
        elif conditional or int(node.get("estimated_cost", 0)) > int(request["budget_units"]) - spent: unresolved.append(nid); omissions.add(f"{nid}:budget-ceiling") if int(node.get("estimated_cost", 0)) > int(request["budget_units"]) - spent else None
        else: completed.append(nid); spent += int(node["estimated_cost"])
    disposition = "blocked" if global_block or cycle or missing else "unresolved" if unresolved or uncertainty else "qualified"; exchange = [f"exchange:permitted-artifacts:{nid}" for nid in completed] if disposition == "qualified" else []; checkpoint = _hash({"workflow_id": request["workflow_id"], "topological_order": topological, "replay_identity": request["replay_identity"]}); payload = {"schema_version": OUTPUT_SCHEMA, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "plan_order": ids, "topological_order": topological, "completed_order": completed, "unresolved_order": unresolved, "blocked_order": blocked, "cycle_order": cycle, "missing_dependency_order": sorted(missing), "study_order": [str(value) for value in request["required_study_order"]], "modality_order": [str(value) for value in request["required_modality_order"]], "exchange_order": exchange, "checkpoint_digest": checkpoint, "replay_identity": request["replay_identity"], "disposition": disposition}; digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"execution-run:{request['workflow_id']}", "content_type": "application/vnd.aurora.execution-run+json", "content_hash": digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": request["workflow_id"], "relation": "graph-execution-gateway", "digest": digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = ExecutionRun(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["workflow_id"]), str(request["scope"]), str(request["semantic_profile"]), disposition, tuple(ids), tuple(topological), tuple(sorted(completed)), tuple(sorted(unresolved)), tuple(sorted(blocked)), tuple(cycle), tuple(sorted(missing)), tuple(str(value) for value in request["required_study_order"]), tuple(str(value) for value in request["required_modality_order"]), tuple(exchange), str(request["replay_identity"]), checkpoint, digest, tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, (f"exchange:permitted-artifacts:{request['workflow_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt

__all__ = ["ExecutionRun", "admit_computational_execution", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
