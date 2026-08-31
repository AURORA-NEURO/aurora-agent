"""Python parity for ``AFA-worldfactory-P12-F32``.

The control plane admits declared computation for a governed local runtime; it does not execute
code, contact instruments, or export raw research data.
"""
from __future__ import annotations
import hashlib, json, re
from dataclasses import dataclass
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-worldfactory-P12-F32"
CONTRACT_VERSION = "worldfactory-federated-continual-computational-execution-federated-control-plane/1.0"
INPUT_SCHEMA = "ComputationalExecutionPlan4@1"
OUTPUT_SCHEMA = "ComputationalExecutionRun9@1"
CONTENT_TYPE = "application/vnd.aurora.computational-execution-run-9+json"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))

@dataclass(frozen=True)
class ComputationalExecutionRun9:
    value: dict[str, Any]
    def to_dict(self) -> dict[str, Any]: return dict(self.value)
    def validate(self) -> None:
        v = self.value; a = v.get("artifact", {})
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or a.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or v.get("checkpoint", 0) <= 0 or v.get("disposition") not in {"qualified", "unresolved", "blocked"} or not all(str(v.get(k, "")).strip() for k in ("request_id", "federation_id", "workflow_id", "requester", "purpose", "semantic_profile")) or not v.get("task_order") or not v.get("peer_order") or not v.get("effect_receipts"): raise ResearchContractError("execution identity, checkpoint, locality, tasks, peers, disposition, or effects are incomplete")
        fields = ("task_order", "admitted_task_order", "unresolved_task_order", "blocked_task_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order", "adversarial_event_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields): raise ResearchContractError("execution ordering is not canonical")
        if set(v["task_order"]) != set(v["admitted_task_order"]) | set(v["unresolved_task_order"]) | set(v["blocked_task_order"]): raise ResearchContractError("execution task dispositions do not partition")
        if set(v["peer_order"]) != set(v["qualified_peer_order"]) | set(v["missing_peer_order"]): raise ResearchContractError("execution peer dispositions do not partition")
        if a.get("content_type") != CONTENT_TYPE or a.get("content_hash") != v.get("execution_digest") or not all(_digest(x) for x in [v.get("replay_identity"), v.get("execution_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]): raise ResearchContractError("execution artifact metadata or digest is invalid")
        if any(not e.startswith(("authorize:local-computation:", "exchange:permitted-execution-summaries:")) and e != "block:unsafe-execution" for e in v["effect_receipts"]): raise ResearchContractError("execution effect is outside governed gate")

def computational_execution_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "worldfactory", "consumers": ["computational researcher", "workflow operator", "federation steward", "runtime executor"], "behavior": "qualifies a declared computational workflow for bounded institution-local execution", "value": "turns execution readiness, provenance, replay, quorum, and policy into an auditable authorization receipt", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["authorize:local-computation", "exchange:permitted-execution-summaries"], "permissions": ["operate:institution-node", "authorize:research-computation"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}

def authorize_computational_execution(plan: Mapping[str, Any]) -> ComputationalExecutionRun9:
    req = plan
    if not all(str(req.get(k, "")).strip() for k in ("request_id", "federation_id", "workflow_id", "requester", "purpose", "semantic_profile", "required_runtime_version")) or int(req.get("checkpoint", 0)) <= 0 or not req.get("tasks") or not req.get("peers") or int(req.get("max_budget_units", 0)) <= 0 or int(req.get("minimum_peer_quorum", 0)) <= 0 or req.get("boundary") != PRECLINICAL_BOUNDARY or req.get("raw_data_local") is not True or req.get("aggregate_only") is not True or not _digest(req.get("replay_identity")): raise ResearchContractError("execution request identity, bounds, tasks, peers, budget, replay, locality, or boundary is invalid")
    tasks = sorted((dict(x) for x in req["tasks"]), key=lambda x: (int(x.get("sequence", 0)), str(x.get("task_id", ""))))
    task_ids = [str(x.get("task_id", "")) for x in tasks]
    if len(set(task_ids)) != len(task_ids) or any(not x.get("task_id") or not x.get("input_schema") or not x.get("output_schema") or not x.get("effect_class") or int(x.get("estimated_units", 0)) <= 0 or not all(_digest(x.get(k)) for k in ("artifact_digest", "provenance_digest", "replay_digest")) for x in tasks): raise ResearchContractError("task identity, schemas, bounds, or digests are invalid")
    peers = sorted((dict(x) for x in req["peers"]), key=lambda x: str(x.get("peer_id", ""))); peer_ids = [str(x.get("peer_id", "")) for x in peers]
    if len(set(peer_ids)) != len(peer_ids) or any(not x.get("peer_id") or not x.get("origin") or not x.get("workflow_id") or not _digest(x.get("run_digest")) for x in peers): raise ResearchContractError("peer identity, origin, workflow, or digest is invalid")
    admitted: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); adversarial: set[str] = set(); total = 0
    for task in tasks:
        tid = task["task_id"]; total += int(task["estimated_units"]); reasons = []; state = task.get("evidence_state")
        if state == "contradicted": reasons.append("contradicted-evidence"); negative.add(f"task:{tid}:contradicted")
        if state not in {"proven", "supported"}: reasons.append("evidence-state-unresolved"); uncertainty.add(f"task:{tid}:evidence-state")
        if task.get("deterministic") is not True: reasons.append("nondeterministic-task"); adversarial.add(f"task:{tid}:nondeterministic")
        if task.get("local_only") is not True: reasons.append("task-not-local")
        if task.get("requires_approval") is True and req.get("signed_approval") is not True: reasons.append("task-approval-missing"); uncertainty.add(f"task:{tid}:approval-missing")
        if not task.get("required_capabilities"): reasons.append("capability-closure-missing"); omissions.add(f"task:{tid}:capability-closure-missing")
        (blocked if any(r in {"contradicted-evidence", "task-not-local"} for r in reasons) else unresolved if reasons else admitted).add(tid)
    qualified_peers = {x["peer_id"] for x in peers if x.get("workflow_id") == req["workflow_id"] and x.get("semantic_profile") == req["semantic_profile"] and int(x.get("checkpoint", 0)) == int(req["checkpoint"]) and x.get("signed") is True and x.get("aggregate_only") is True and x.get("raw_data_local") is True and x.get("evidence_state") in {"proven", "supported"}}
    missing_peers = set(peer_ids) - qualified_peers; uncertainty.update(f"peer:{x}:not-qualified" for x in missing_peers); negative.update(f"peer:{x['peer_id']}:contradicted" for x in peers if x.get("evidence_state") == "contradicted")
    global_block = not all(req.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "federation_approved", "raw_data_local", "aggregate_only"))
    if req.get("policy_allow") is not True: negative.add("request:policy-denied")
    if req.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if req.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if req.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    if total > int(req["max_budget_units"]): omissions.add("request:budget-exceeded")
    if len(qualified_peers) < int(req["minimum_peer_quorum"]): uncertainty.add("peer:minimum-quorum-unmet")
    disposition = "blocked" if global_block or blocked or total > int(req["max_budget_units"]) else "unresolved" if len(qualified_peers) < int(req["minimum_peer_quorum"]) or unresolved or not admitted else "qualified"
    if disposition != "qualified": omissions.add("request:execution-not-release-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": req["request_id"], "federation_id": req["federation_id"], "workflow_id": req["workflow_id"], "requester": req["requester"], "purpose": req["purpose"], "semantic_profile": req["semantic_profile"], "checkpoint": int(req["checkpoint"]), "disposition": disposition, "task_order": task_ids, "admitted_task_order": sorted(admitted), "unresolved_task_order": sorted(unresolved), "blocked_task_order": sorted(blocked), "peer_order": peer_ids, "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "adversarial_event_order": sorted(adversarial), "total_units": total, "replay_identity": req["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); result = {**payload, "execution_digest": digest, "artifact": {"artifact_id": f"computational-execution-run-9:{req['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance_digests": sorted({x["provenance_digest"] for x in tasks}), "boundary": PRECLINICAL_BOUNDARY}, "effect_receipts": [f"authorize:local-computation:{req['request_id']}", f"exchange:permitted-execution-summaries:{req['request_id']}"] if disposition == "qualified" else ["block:unsafe-execution"], "raw_data_local": bool(req["raw_data_local"]), "aggregate_only": bool(req["aggregate_only"])}
    receipt = ComputationalExecutionRun9(result); receipt.validate(); return receipt

def computationalExecutionRun9Digest(receipt: ComputationalExecutionRun9) -> str:
    receipt.validate(); return _hash(receipt.to_dict())

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "ComputationalExecutionRun9", "computational_execution_manifest", "authorize_computational_execution", "computationalExecutionRun9Digest"]
