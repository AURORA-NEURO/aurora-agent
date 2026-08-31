"""Institution-local context control-plane parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class LocalContextControlStage:
    stage_id: str
    context_digest: str
    section_digest: str
    evidence_digest: str | None
    provenance_digest: str | None
    replay_identity: str
    state: str = "supported"
    ready: bool = True
    retry_count: int = 0
    telemetry_digest: str | None = None
    cost_units: int = 1
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class LocalContextControlReceipt:
    request_id: str
    node_id: str
    workflow_id: str
    scope: str
    goal: str
    disposition: str
    stage_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    degraded_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    exchange_order: tuple[str, ...]
    checkpoint_seq: int
    retry_count: int
    consumed_budget_units: int
    run_digest: str
    telemetry_digest: str
    federation_digest: str
    replay_identity: str
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID
    contract_version: str = LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID or self.contract_version != LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION:
            raise ResearchContractError("local control schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.node_id.strip() or not self.workflow_id.strip() or not self.scope.strip() or not self.goal.strip() or not self.stage_order or self.checkpoint_seq != len(self.stage_order) or not self.effect_receipts or self.disposition not in {"completed", "degraded", "unresolved", "denied"}:
            raise ResearchContractError("local control identity, checkpoint, locality, disposition, or effects are incomplete")
        for values in (self.stage_order, self.completed_order, self.degraded_order, self.unresolved_order, self.denied_order, self.exchange_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("local control ordering is not canonical")
        classified = set(self.completed_order) | set(self.degraded_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.stage_order):
            raise ResearchContractError("local control dispositions do not partition stages")
        if len(self.exchange_order) != len(self.completed_order):
            raise ResearchContractError("local control exchange does not match completed stages")
        for value in (*self.exchange_order, self.run_digest, self.telemetry_digest, self.federation_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("local control digest is invalid")
        if any(not effect.startswith("exchange:permitted-context-summary:") and not effect.startswith("manage:local-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("local control effect is outside the governed operations gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "node_id": self.node_id, "workflow_id": self.workflow_id, "scope": self.scope, "goal": self.goal, "disposition": self.disposition, "stage_order": list(self.stage_order), "completed_order": list(self.completed_order), "degraded_order": list(self.degraded_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "exchange_order": list(self.exchange_order), "checkpoint_seq": self.checkpoint_seq, "retry_count": self.retry_count, "consumed_budget_units": self.consumed_budget_units, "run_digest": self.run_digest, "telemetry_digest": self.telemetry_digest, "federation_digest": self.federation_digest, "replay_identity": self.replay_identity, "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def operate_local_context_compilation(*, request_id: str, node_id: str, workflow_id: str, scope: str, goal: str, stage_ids: Sequence[str], stages: Sequence[LocalContextControlStage], max_retries: int, budget_units: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, signed_approval: bool = True) -> LocalContextControlReceipt:
    if not request_id.strip() or not node_id.strip() or not workflow_id.strip() or not scope.strip() or not goal.strip() or not stage_ids or not stages or budget_units <= 0 or max_retries < 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("local control identity, stages, budget, replay, or boundary is invalid")
    ordered = tuple(sorted(set(stage_ids)))
    if len(ordered) != len(stage_ids) or any(not value.strip() for value in ordered):
        raise ResearchContractError("local control stage identifiers must be unique and non-empty")
    stage_map: dict[str, LocalContextControlStage] = {}
    for stage in stages:
        if stage.stage_id in stage_map:
            raise ResearchContractError("local control stages must be unique")
        stage_map[stage.stage_id] = stage
    completed: set[str] = set(); degraded: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); exchanges: list[str] = []
    witnesses = {"gate:typed-local-control-contract", "gate:checkpoint", "gate:bounded-retry", "gate:telemetry", "gate:provenance", "gate:replay-identity", "gate:locality", "gate:permitted-summary"}; counter: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    open_gate = policy_allow and protected_closure and raw_data_local and signed_approval; consumed = 0; retries = 0
    for stage_id in ordered:
        stage = stage_map.get(stage_id)
        if stage is None: unresolved.add(stage_id); omissions.add(f"stage:{stage_id}:missing-checkpoint")
        elif not open_gate or not stage.raw_data_local or stage.boundary != PRECLINICAL_BOUNDARY: denied.add(stage_id); counter.add(f"counterexample:{stage_id}:policy-approval-locality")
        elif stage.retry_count > max_retries: degraded.add(stage_id); omissions.add(f"stage:{stage_id}:retry-budget-exhausted")
        elif consumed + stage.cost_units > budget_units: denied.add(stage_id); omissions.add(f"stage:{stage_id}:resource-budget-exhausted")
        elif not stage.ready: unresolved.add(stage_id); uncertainty.add(f"stage:{stage_id}:not-ready")
        elif stage.replay_identity != replay_identity: unresolved.add(stage_id); uncertainty.add(f"stage:{stage_id}:replay-mismatch")
        elif stage.telemetry_digest is None: unresolved.add(stage_id); omissions.add(f"stage:{stage_id}:telemetry-missing")
        elif stage.evidence_digest is None or stage.provenance_digest is None: unresolved.add(stage_id); omissions.add(f"stage:{stage_id}:evidence-or-provenance-missing")
        elif stage.state in {"unknown", "speculative"}: unresolved.add(stage_id); uncertainty.add(f"stage:{stage_id}:evidence-uncertain")
        elif stage.state == "contradicted": denied.add(stage_id); negative.add(f"stage:{stage_id}:contradicted")
        else:
            completed.add(stage_id); consumed += stage.cost_units; exchanges.append(research_artifact_digest({"stage_id": stage.stage_id, "context_digest": stage.context_digest, "section_digest": stage.section_digest, "evidence_digest": stage.evidence_digest, "provenance_digest": stage.provenance_digest, "telemetry_digest": stage.telemetry_digest}))
        if stage is not None: retries += stage.retry_count
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("control:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("control:protected-closure-incomplete")
    if not signed_approval: counter.add("counterexample:signed-approval-missing"); omissions.add("control:signed-approval-missing")
    if unresolved or degraded: witnesses.add("gate:degraded-or-unresolved-retained")
    exchange_order = tuple(sorted(exchanges)); disposition = "denied" if not open_gate or denied else "unresolved" if unresolved else "degraded" if degraded else "completed"
    telemetry = research_artifact_digest({"feature_id": LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "workflow_id": workflow_id, "stage_order": list(ordered), "retry_count": retries, "exchange_order": list(exchange_order)})
    federation = research_artifact_digest({"node_id": node_id, "workflow_id": workflow_id, "exchange_order": list(exchange_order), "raw_data_local": raw_data_local, "replay_identity": replay_identity})
    run = research_artifact_digest({"feature_id": LOCAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "request_id": request_id, "disposition": disposition, "completed_order": sorted(completed), "degraded_order": sorted(degraded), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "checkpoint_seq": len(ordered), "consumed_budget_units": consumed, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": replay_identity})
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "run_digest": run}), "media_type": "application/vnd.aurora.local-context-control+json"}
    receipt = LocalContextControlReceipt(request_id=request_id, node_id=node_id, workflow_id=workflow_id, scope=scope, goal=goal, disposition=disposition, stage_order=ordered, completed_order=tuple(sorted(completed)), degraded_order=tuple(sorted(degraded)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), exchange_order=exchange_order, checkpoint_seq=len(ordered), retry_count=retries, consumed_budget_units=consumed, run_digest=run, telemetry_digest=telemetry, federation_digest=federation, replay_identity=replay_identity, witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counter)), omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"exchange:permitted-context-summary:{request_id}", f"manage:local-context:{request_id}") if disposition == "completed" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
